use std::cell::OnceCell;

use clipboard::{windows_clipboard::WindowsClipboardContext, ClipboardProvider};
use egui::{epaint::Primitive, Context};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::Direct3D9::{IDirect3DDevice9, D3DPT_TRIANGLELIST, D3DVIEWPORT9},
    UI::WindowsAndMessaging::GetClientRect,
};

use crate::{
    inputman::InputManager,
    mesh::{Buffers, GpuVertex, MeshDescriptor},
    state::DxState,
    texman::TextureManager,
};

pub struct EguiDx9<T> {
    ui_fn: Box<dyn FnMut(&Context, &mut T) + 'static>,
    ui_state: T,
    hwnd: OnceCell<HWND>,
    input_man: InputManager,
    // get it? tEx-man? tax-man? no?
    tex_man: TextureManager,
    ctx: Context,
    buffers: Buffers,
    last_idx_capacity: usize,
    last_vtx_capacity: usize,
}

impl<T> EguiDx9<T> {
    pub fn init(
        dev: &IDirect3DDevice9,
        hwnd: HWND,
        ui_fn: impl FnMut(&Context, &mut T) + 'static,
        ui_state: T,
    ) -> Self {
        Self {
            ui_fn: Box::new(ui_fn),
            ui_state,
            hwnd: OnceCell::from(hwnd),
            tex_man: TextureManager::new(),
            input_man: InputManager::new(hwnd),
            ctx: Context::default(),
            buffers: Buffers::create_buffers(dev, 16384, 16384),
            last_idx_capacity: 0,
            last_vtx_capacity: 0,
        }
    }

    pub fn present(&mut self, dev: &IDirect3DDevice9) {
        // back up our state so we don't mess with the game and the game doesn't mess with us.
        // i actually had the idea to use BeginStateBlock and co. to "cache" the state we set every frame,
        // and just re-applying it everytime. just setting this manually takes around 50 microseconds on my machine.
        let _state = DxState::setup(dev, self.get_viewport());

        let output = self.ctx.run(self.input_man.collect_input(), |ctx| {
            // safe. present will never run in parallel.
            (self.ui_fn)(ctx, &mut self.ui_state)
        });

        if !output.textures_delta.is_empty() {
            self.tex_man.process_set_deltas(dev, &output.textures_delta);
        }

        if !output.platform_output.copied_text.is_empty() {
            let _ = WindowsClipboardContext.set_contents(output.platform_output.copied_text);
        }

        if output.shapes.is_empty() {
            // early return, don't forget to free textures
            if !output.textures_delta.is_empty() {
                self.tex_man.process_free_deltas(&output.textures_delta);
            }
            return;
        }

        let mut vertices: Vec<GpuVertex> = Vec::with_capacity(self.last_vtx_capacity + 512);
        let mut indices: Vec<u32> = Vec::with_capacity(self.last_idx_capacity + 512);

        let prims: Vec<MeshDescriptor> = self
            .ctx
            .tessellate(output.shapes)
            .into_iter()
            .filter_map(|prim| {
                if let Primitive::Mesh(mesh) = prim.primitive {
                    // most definitely not the rusty way to do this.
                    // it's ugly, but its efficient.
                    if let Some((gpumesh, verts, idxs)) =
                        MeshDescriptor::from_mesh(mesh, prim.clip_rect)
                    {
                        vertices.extend_from_slice(verts.as_slice());
                        indices.extend_from_slice(idxs.as_slice());

                        Some(gpumesh)
                    } else {
                        None
                    }
                } else {
                    panic!("paint callbacks not supported")
                }
            })
            .collect();

        self.last_vtx_capacity = vertices.len();
        self.last_idx_capacity = indices.len();

        self.buffers.update_vertex_buffer(dev, &vertices);
        self.buffers.update_index_buffer(dev, &indices);

        unsafe {
            expect!(
                dev.SetStreamSource(
                    0,
                    &self.buffers.vtx,
                    0,
                    std::mem::size_of::<GpuVertex>() as _
                ),
                "unable to set vertex stream source"
            );

            expect!(
                dev.SetIndices(&self.buffers.idx),
                "unable to set index buffer"
            );
        }

        let mut our_vtx_idx: usize = 0;
        let mut our_idx_idx: usize = 0;

        prims.iter().for_each(|mesh: &MeshDescriptor| unsafe {
            expect!(
                dev.SetScissorRect(&RECT {
                    left: mesh.clip.left() as _,
                    top: mesh.clip.top() as _,
                    right: mesh.clip.right() as _,
                    bottom: mesh.clip.bottom() as _,
                }),
                "unable to set scissor rect"
            );

            let texture = self.tex_man.get_by_id(mesh.texture_id);

            expect!(dev.SetTexture(0, texture), "unable to set texture");

            expect!(
                dev.DrawIndexedPrimitive(
                    D3DPT_TRIANGLELIST,
                    our_vtx_idx as _,
                    0,
                    mesh.vertices as _,
                    our_idx_idx as _,
                    (mesh.indices / 3usize) as _
                ),
                "unable to draw indexed prims"
            );

            our_vtx_idx = our_vtx_idx + mesh.vertices;
            our_idx_idx = our_idx_idx + mesh.indices;
        });

        if !output.textures_delta.is_empty() {
            self.tex_man.process_free_deltas(&output.textures_delta);
        }
    }

    #[inline]
    pub fn wnd_proc(&mut self, umsg: u32, wparam: WPARAM, lparam: LPARAM) {
        // safe. we only write here, and only read elsewhere.
        self.input_man.process(umsg, wparam.0, lparam.0);
    }
}

impl<T> EguiDx9<T> {
    #[inline]
    fn get_screen_size(&self) -> (f32, f32) {
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(
                *expect!(self.hwnd.get(), "You need to call init first"),
                &mut rect,
            );
        }
        (
            (rect.right - rect.left) as f32,
            (rect.bottom - rect.top) as f32,
        )
    }

    #[inline]
    fn get_viewport(&self) -> D3DVIEWPORT9 {
        let (w, h) = self.get_screen_size();
        D3DVIEWPORT9 {
            X: 0,
            Y: 0,
            Width: w as _,
            Height: h as _,
            MinZ: 0.,
            MaxZ: 1.,
        }
    }
}
