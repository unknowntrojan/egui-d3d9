use egui::{Color32, Mesh, Pos2, Rect, TextureId};
use windows::Win32::{
    Foundation::{HANDLE, RECT},
    Graphics::Direct3D9::{
        IDirect3DDevice9, IDirect3DIndexBuffer9, IDirect3DVertexBuffer9, D3DFMT_INDEX32,
        D3DFVF_DIFFUSE, D3DFVF_TEX1, D3DFVF_XYZ, D3DLOCK_DISCARD, D3DPOOL_DEFAULT,
        D3DUSAGE_DYNAMIC, D3DUSAGE_WRITEONLY,
    },
};

// XYZ is 32 bits completely wasted per vertex.
// but that's the cost of doing business, I really cba dealing with shaders again
// although I'll probably do it at some point
pub const FVF_CUSTOMVERTEX: u32 = D3DFVF_XYZ | D3DFVF_DIFFUSE | D3DFVF_TEX1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VertexColor {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

impl From<Color32> for VertexColor {
    fn from(value: Color32) -> Self {
        let rgba = value.to_tuple();
        Self {
            r: rgba.0,
            g: rgba.1,
            b: rgba.2,
            a: rgba.3,
        }
    }
}

pub struct MeshDescriptor {
    pub vertices: usize,
    pub indices: usize,
    pub clip: RECT,
    pub texture_id: TextureId,
}

impl MeshDescriptor {
    pub fn from_mesh(mesh: Mesh, scissors: Rect) -> Option<(Self, Vec<GpuVertex>, Vec<u32>)> {
        if mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            None
        } else {
            let vertices: Vec<GpuVertex> = mesh
                .vertices
                .into_iter()
                .map(|v| GpuVertex {
                    pos: [v.pos.x, v.pos.y, 0f32],
                    uv: v.uv,
                    color: v.color.into(),
                })
                .collect();

            Some((
                Self {
                    vertices: vertices.len(),
                    indices: mesh.indices.len(),
                    clip: RECT {
                        left: scissors.left() as _,
                        top: scissors.top() as _,
                        right: scissors.right() as _,
                        bottom: scissors.bottom() as _,
                    },
                    texture_id: mesh.texture_id,
                },
                vertices,
                mesh.indices,
            ))
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GpuVertex {
    pos: [f32; 3],
    color: VertexColor,
    uv: Pos2,
}

pub struct Buffers {
    pub vtx: Option<IDirect3DVertexBuffer9>,
    pub idx: Option<IDirect3DIndexBuffer9>,
    vtx_size: usize,
    idx_size: usize,
}

impl Buffers {
    pub fn create_buffers(
        device: &IDirect3DDevice9,
        vtx_count: usize,
        idx_count: usize,
    ) -> Buffers {
        Buffers {
            vtx_size: vtx_count,
            idx_size: idx_count,
            vtx: Some(Self::create_vertex_buffer(device, vtx_count)),
            idx: Some(Self::create_index_buffer(device, idx_count)),
        }
    }

    pub fn delete_buffers(&mut self) {
        self.vtx = None;
        self.idx = None;
    }

    fn create_vertex_buffer(device: &IDirect3DDevice9, vertices: usize) -> IDirect3DVertexBuffer9 {
        unsafe {
            let mut vertex_buffer: Option<IDirect3DVertexBuffer9> = None;
            expect!(
                device.CreateVertexBuffer(
                    (vertices * std::mem::size_of::<GpuVertex>()) as u32,
                    (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as _,
                    FVF_CUSTOMVERTEX,
                    D3DPOOL_DEFAULT,
                    &mut vertex_buffer,
                    std::ptr::null_mut::<HANDLE>()
                ),
                "Failed to create vertex buffer"
            );

            expect!(vertex_buffer, "unable to create vertex buffer")
        }
    }

    fn create_index_buffer(device: &IDirect3DDevice9, indices: usize) -> IDirect3DIndexBuffer9 {
        unsafe {
            let mut index_buffer: Option<IDirect3DIndexBuffer9> = None;
            expect!(
                device.CreateIndexBuffer(
                    (indices * std::mem::size_of::<u32>()) as u32,
                    (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as _,
                    D3DFMT_INDEX32,
                    D3DPOOL_DEFAULT,
                    &mut index_buffer,
                    std::ptr::null_mut::<HANDLE>()
                ),
                "Failed to create index buffer"
            );

            expect!(index_buffer, "unable to create index buffer")
        }
    }

    pub fn update_vertex_buffer(&mut self, device: &IDirect3DDevice9, vertices: &[GpuVertex]) {
        unsafe {
            let buf_len = vertices.len();

            if self.vtx_size < buf_len {
                let new_size = buf_len + 1024;
                self.vtx = Some(Self::create_vertex_buffer(device, new_size));
                self.vtx_size = new_size;
            }

            let vtx = expect!(self.vtx.as_mut(), "unable to get vertex buffer");

            let mut buffer: *mut GpuVertex = std::mem::zeroed();

            expect!(
                vtx.Lock(
                    0,
                    vertices.len() as u32 * std::mem::size_of::<GpuVertex>() as u32,
                    std::mem::transmute(&mut buffer),
                    D3DLOCK_DISCARD as _
                ),
                "unable to lock vertex buffer"
            );

            let buffer = std::slice::from_raw_parts_mut(buffer, vertices.len() as _);

            buffer.copy_from_slice(vertices);

            expect!(vtx.Unlock(), "unable to unlock vtx buffer");
        }
    }

    pub fn update_index_buffer(&mut self, device: &IDirect3DDevice9, indices: &[u32]) {
        unsafe {
            let buf_len = indices.len();

            if self.idx_size < buf_len {
                let new_size = buf_len + 1024;
                self.idx = Some(Self::create_index_buffer(device, new_size));
                self.idx_size = new_size;
            }

            let idx = expect!(self.idx.as_mut(), "unable to get index buffer");

            let mut buffer: *mut u32 = std::mem::zeroed();

            expect!(
                idx.Lock(
                    0,
                    indices.len() as u32 * std::mem::size_of::<u32>() as u32,
                    &mut buffer as *mut *mut u32 as *mut *mut std::ffi::c_void,
                    D3DLOCK_DISCARD as _
                ),
                "unable to lock index buffer"
            );

            let buffer = std::slice::from_raw_parts_mut(buffer, indices.len() as _);

            buffer.copy_from_slice(indices);

            expect!(idx.Unlock(), "unable to unlock idx buffer");
        }
    }
}
