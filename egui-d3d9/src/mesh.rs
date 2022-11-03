use egui::{Color32, Mesh, Pos2, Rect, TextureId};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D9::{
        IDirect3DDevice9, IDirect3DIndexBuffer9, IDirect3DVertexBuffer9, D3DFMT_INDEX32,
        D3DLOCK_DISCARD, D3DPOOL_DEFAULT, D3DUSAGE_DYNAMIC, D3DUSAGE_WRITEONLY,
    },
    System::SystemServices::{D3DFVF_DIFFUSE, D3DFVF_TEX1, D3DFVF_XYZ},
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

pub struct GpuMesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<GpuVertex>,
    pub clip: Rect,
    pub texture_id: TextureId,
}

impl GpuMesh {
    pub fn from_mesh(mesh: Mesh, scissors: Rect) -> Option<Self> {
        if mesh.indices.is_empty() || mesh.indices.len() % 3 != 0 {
            None
        } else {
            let vertices = mesh
                .vertices
                .into_iter()
                .map(|v| GpuVertex {
                    pos: [v.pos.x, v.pos.y, 0f32],
                    uv: v.uv,
                    color: v.color.into(),
                })
                .collect();

            Some(Self {
                texture_id: mesh.texture_id,
                indices: mesh.indices,
                clip: scissors,
                vertices,
            })
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
    pub vtx: IDirect3DVertexBuffer9,
    pub idx: IDirect3DIndexBuffer9,
}

pub fn create_buffers(dev: &IDirect3DDevice9, mesh: &GpuMesh) -> Buffers {
    Buffers {
        vtx: create_vertex_buffer(dev, mesh),
        idx: create_index_buffer(dev, mesh),
    }
}

fn create_vertex_buffer(device: &IDirect3DDevice9, mesh: &GpuMesh) -> IDirect3DVertexBuffer9 {
    unsafe {
        let mut vertex_buffer: Option<IDirect3DVertexBuffer9> = None;
        expect!(
            device.CreateVertexBuffer(
                (mesh.vertices.len() * std::mem::size_of::<GpuVertex>()) as u32,
                (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as _,
                FVF_CUSTOMVERTEX,
                D3DPOOL_DEFAULT,
                &mut vertex_buffer,
                std::ptr::null_mut::<HANDLE>()
            ),
            "Failed to create vertex buffer"
        );

        let vertex_buffer = expect!(vertex_buffer, "unable to create vertex buffer");

        let mut buffer: *mut GpuVertex = std::mem::zeroed();

        expect!(
            vertex_buffer.Lock(
                0,
                mesh.vertices.len() as u32 * std::mem::size_of::<GpuVertex>() as u32,
                std::mem::transmute(&mut buffer),
                D3DLOCK_DISCARD as _
            ),
            "unable to lock vertex buffer"
        );

        let buffer = std::slice::from_raw_parts_mut(buffer, mesh.vertices.len() as _);

        buffer.copy_from_slice(mesh.vertices.as_slice());

        expect!(vertex_buffer.Unlock(), "unable to unlock vtx buffer");

        vertex_buffer
    }
}

fn create_index_buffer(device: &IDirect3DDevice9, mesh: &GpuMesh) -> IDirect3DIndexBuffer9 {
    unsafe {
        let mut index_buffer: Option<IDirect3DIndexBuffer9> = None;
        expect!(
            device.CreateIndexBuffer(
                (mesh.indices.len() * std::mem::size_of::<u32>()) as u32,
                (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as _,
                D3DFMT_INDEX32,
                D3DPOOL_DEFAULT,
                &mut index_buffer,
                std::ptr::null_mut::<HANDLE>()
            ),
            "Failed to create index buffer"
        );

        let index_buffer = expect!(index_buffer, "unable to create index buffer");

        let mut buffer: *mut u32 = std::mem::zeroed();

        expect!(
            index_buffer.Lock(
                0,
                mesh.indices.len() as u32 * std::mem::size_of::<u32>() as u32,
                std::mem::transmute(&mut buffer),
                D3DLOCK_DISCARD as _
            ),
            "unable to lock index buffer"
        );

        let buffer = std::slice::from_raw_parts_mut(buffer, mesh.indices.len() as _);

        buffer.copy_from_slice(mesh.indices.as_slice());

        expect!(index_buffer.Unlock(), "unable to unlock idx buffer");

        index_buffer
    }
}
