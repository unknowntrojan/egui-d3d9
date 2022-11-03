use std::cell::OnceCell;

use clipboard::{windows_clipboard::WindowsClipboardContext, ClipboardProvider};
use egui::{epaint::Primitive, Context};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    Graphics::{
        Direct3D::{D3DMATRIX, D3DMATRIX_0},
        Direct3D9::{
            IDirect3DDevice9, IDirect3DSurface9, D3DBACKBUFFER_TYPE_MONO, D3DBLENDOP_ADD,
            D3DBLEND_INVSRCALPHA, D3DBLEND_ONE, D3DBLEND_SRCALPHA, D3DCULL_NONE, D3DFILL_SOLID,
            D3DPT_TRIANGLELIST, D3DRS_ALPHABLENDENABLE, D3DRS_ALPHATESTENABLE, D3DRS_BLENDOP,
            D3DRS_CLIPPING, D3DRS_CULLMODE, D3DRS_DESTBLEND, D3DRS_DESTBLENDALPHA, D3DRS_FILLMODE,
            D3DRS_FOGENABLE, D3DRS_LIGHTING, D3DRS_RANGEFOGENABLE, D3DRS_SCISSORTESTENABLE,
            D3DRS_SEPARATEALPHABLENDENABLE, D3DRS_SHADEMODE, D3DRS_SPECULARENABLE, D3DRS_SRCBLEND,
            D3DRS_SRCBLENDALPHA, D3DRS_STENCILENABLE, D3DRS_ZENABLE, D3DRS_ZWRITEENABLE,
            D3DSAMP_MAGFILTER, D3DSAMP_MINFILTER, D3DSAMP_MIPFILTER, D3DSHADE_GOURAUD,
            D3DTEXF_LINEAR, D3DTOP_DISABLE, D3DTOP_MODULATE, D3DTSS_ALPHAARG1, D3DTSS_ALPHAARG2,
            D3DTSS_ALPHAOP, D3DTSS_COLORARG1, D3DTSS_COLORARG2, D3DTSS_COLOROP, D3DTS_PROJECTION,
            D3DTS_VIEW, D3DTS_WORLD, D3DVIEWPORT9,
        },
    },
    System::SystemServices::{D3DTA_DIFFUSE, D3DTA_TEXTURE},
    UI::WindowsAndMessaging::GetClientRect,
};

use crate::{
    backup,
    inputman::InputManager,
    mesh::{create_buffers, GpuMesh, GpuVertex, FVF_CUSTOMVERTEX},
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
        // back up our state so we don't mess with the game.
        let _state_block = backup::StateBackup::new(dev);

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

        self.setup_state(dev);

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

        prims.iter().for_each(|mesh: &GpuMesh| unsafe {
            let buffers = create_buffers(dev, mesh);

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
    fn setup_state(&self, dev: &IDirect3DDevice9) {
        // DON'T LOOK! I SPAMMED THESE FOR DEBUG REASONS, THIS SHOULD RETURN A RESULT IN AN IDEAL WORLD

        unsafe {
            // general set up
            let backbuffer: IDirect3DSurface9 = expect!(
                dev.GetBackBuffer(0, 0, D3DBACKBUFFER_TYPE_MONO),
                "failed to get swapchain's backbuffer"
            );

            expect!(
                dev.SetRenderTarget(0, &backbuffer),
                "unable to set the render target to the back buffer"
            );

            expect!(
                dev.SetViewport(&self.get_viewport()),
                "unable to set viewport"
            );

            // set up fvf
            expect!(dev.SetPixelShader(None), "unable to unset pxl shader");
            expect!(dev.SetVertexShader(None), "unable to unset vtx shader");
            expect!(dev.SetFVF(FVF_CUSTOMVERTEX), "unable to set fvf");

            let screen_size = self.get_screen_size();

            // set up matrix
            let l = 0.5;
            let r = screen_size.0 + 0.5;
            let t = 0.5;
            let b = screen_size.1 + 0.5;

            let mat_ident = D3DMATRIX {
                Anonymous: D3DMATRIX_0 {
                    m: [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ],
                },
            };

            let mat_proj = D3DMATRIX {
                Anonymous: D3DMATRIX_0 {
                    m: [
                        2.0 / (r - l),
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        2.0 / (t - b),
                        0.0,
                        0.0,
                        0.0,
                        0.0,
                        0.5,
                        0.0,
                        (l + r) / (l - r),
                        (t + b) / (b - t),
                        0.5,
                        1.0,
                    ],
                },
            };

            expect!(
                dev.SetTransform(D3DTS_WORLD, &mat_ident),
                "unable to set world matrix"
            );

            expect!(
                dev.SetTransform(D3DTS_VIEW, &mat_ident),
                "unable to set view matrix"
            );

            expect!(
                dev.SetTransform(D3DTS_PROJECTION, &mat_proj),
                "unable to set projection matrix"
            );

            // set up render state
            expect!(
                dev.SetRenderState(D3DRS_FILLMODE, D3DFILL_SOLID.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SHADEMODE, D3DSHADE_GOURAUD.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_ZWRITEENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_ALPHATESTENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_ZENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_ALPHABLENDENABLE, true.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, true.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as _),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SCISSORTESTENABLE, true.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_FOGENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_RANGEFOGENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_SPECULARENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_STENCILENABLE, false.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_CLIPPING, true.into()),
                "unable to set render state"
            );
            expect!(
                dev.SetRenderState(D3DRS_LIGHTING, false.into()),
                "unable to set render state"
            );
            // expect!(
            //     dev.SetRenderState(D3DRS_LASTPIXEL, false.into()),
            //     "unable to set render state"
            // );

            // set up texture stages
            expect!(
                dev.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE.0 as _),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as _),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as _),
                "unable to set texture stage state"
            );
            expect!(
                dev.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE.0 as _),
                "unable to set texture stage state"
            );

            // set up sampler
            expect!(
                dev.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as _),
                "unable to set sampler state"
            );
            expect!(
                dev.SetSamplerState(0, D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as _),
                "unable to set sampler state"
            );
            expect!(
                dev.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as _),
                "unable to set sampler state"
            );
        }
    }

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
