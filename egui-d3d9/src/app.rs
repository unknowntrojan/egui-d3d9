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
    mesh::{Buffers, GpuMesh, GpuVertex},
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
}

impl<T> EguiDx9<T> {
    pub fn init(hwnd: HWND, ui_fn: impl FnMut(&Context, &mut T) + 'static, ui_state: T) -> Self {
        Self {
            ui_fn: Box::new(ui_fn),
            ui_state,
            hwnd: OnceCell::from(hwnd),
            tex_man: TextureManager::new(),
            input_man: InputManager::new(hwnd),
            ctx: Context::default(),
        }
    }

    pub fn present(&mut self, dev: &IDirect3DDevice9) {
        // back up our state so we don't mess with the game and the game doesn't mess with us.
        // i actually had the idea to use BeginStateBlock and co. to "cache" the state we set every frame,
        // and just re-applying it everytime. this has a very low performance impact, so it doesn't matter.
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

        let prims: Vec<GpuMesh> = self
            .ctx
            .tessellate(output.shapes)
            .into_iter()
            .filter_map(|prim| {
                if let Primitive::Mesh(mesh) = prim.primitive {
                    GpuMesh::from_mesh(mesh, prim.clip_rect)
                } else {
                    panic!("paint callbacks not supported")
                }
            })
            .collect();

        // instead of only making one buffer and updating it, we could merge all meshes.
        // we could then compute an offset into the buffer and apply new scissor rects
        // etc. simply by knowing which index we're at.
        let (total_vertices, total_indices) = prims.iter().fold((0, 0), |acc, mesh| {
            (
                std::cmp::max(acc.0, mesh.vertices.len()),
                std::cmp::max(acc.1, mesh.indices.len()),
            )
        });

        let mut buffers = Buffers::create_buffers(dev, total_vertices, total_indices);

        prims.iter().for_each(|mesh: &GpuMesh| unsafe {
            buffers.update_buffers(mesh);

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
                dev.SetStreamSource(0, &buffers.vtx, 0, std::mem::size_of::<GpuVertex>() as _),
                "unable to set vertex stream source"
            );

            expect!(dev.SetIndices(&buffers.idx), "unable to set index buffer");

            expect!(
                dev.DrawIndexedPrimitive(
                    D3DPT_TRIANGLELIST,
                    0,
                    0,
                    mesh.vertices.len() as _,
                    0,
                    (mesh.indices.len() / 3usize) as _
                ),
                "unable to draw indexed prims"
            );
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
