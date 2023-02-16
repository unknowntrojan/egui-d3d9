use windows::{
    Foundation::Numerics::Matrix4x4,
    Win32::{
        Graphics::Direct3D9::{
            IDirect3DDevice9, IDirect3DStateBlock9, IDirect3DSurface9, D3DBACKBUFFER_TYPE_MONO,
            D3DBLENDOP_ADD, D3DBLEND_INVSRCALPHA, D3DBLEND_ONE, D3DBLEND_SRCALPHA, D3DCULL_NONE,
            D3DFILL_SOLID, D3DRS_ALPHABLENDENABLE, D3DRS_ALPHATESTENABLE, D3DRS_BLENDOP,
            D3DRS_BLENDOPALPHA, D3DRS_CLIPPING, D3DRS_COLORWRITEENABLE, D3DRS_CULLMODE,
            D3DRS_DESTBLEND, D3DRS_DESTBLENDALPHA, D3DRS_FILLMODE, D3DRS_FOGENABLE,
            D3DRS_LASTPIXEL, D3DRS_LIGHTING, D3DRS_RANGEFOGENABLE, D3DRS_SCISSORTESTENABLE,
            D3DRS_SEPARATEALPHABLENDENABLE, D3DRS_SHADEMODE, D3DRS_SPECULARENABLE, D3DRS_SRCBLEND,
            D3DRS_SRCBLENDALPHA, D3DRS_SRGBWRITEENABLE, D3DRS_STENCILENABLE, D3DRS_TEXTUREFACTOR,
            D3DRS_ZENABLE, D3DRS_ZWRITEENABLE, D3DSAMP_ADDRESSU, D3DSAMP_ADDRESSV,
            D3DSAMP_ADDRESSW, D3DSAMP_BORDERCOLOR, D3DSAMP_MAGFILTER, D3DSAMP_MINFILTER,
            D3DSAMP_MIPFILTER, D3DSBT_ALL, D3DSHADE_GOURAUD, D3DTADDRESS_CLAMP, D3DTEXF_LINEAR,
            D3DTOP_DISABLE, D3DTOP_MODULATE, D3DTSS_ALPHAARG0, D3DTSS_ALPHAARG1, D3DTSS_ALPHAARG2,
            D3DTSS_ALPHAOP, D3DTSS_COLORARG0, D3DTSS_COLORARG1, D3DTSS_COLORARG2, D3DTSS_COLOROP,
            D3DTS_PROJECTION, D3DTS_VIEW, D3DTS_WORLD, D3DVIEWPORT9,
        },
        System::SystemServices::{D3DTA_CURRENT, D3DTA_DIFFUSE, D3DTA_TEXTURE},
    },
};

use crate::mesh::FVF_CUSTOMVERTEX;

pub struct DxState {
    original_state: IDirect3DStateBlock9,
    original_world: Matrix4x4,
    original_view: Matrix4x4,
    original_proj: Matrix4x4,
    dev: IDirect3DDevice9,
}

impl DxState {
    pub fn setup(dev: &IDirect3DDevice9, viewport: D3DVIEWPORT9) -> Self {
        unsafe {
            // backup state
            let original_state = {
                expect!(
                    dev.CreateStateBlock(D3DSBT_ALL),
                    "unable to back up game state"
                )
            };

            expect!(
                original_state.Capture(),
                "unable to capture dx state backup"
            );

            let mut original_world: Matrix4x4 = Default::default();
            let mut original_view: Matrix4x4 = Default::default();
            let mut original_proj: Matrix4x4 = Default::default();

            expect!(
                dev.GetTransform(D3DTS_WORLD, &mut original_world),
                "unable to backup world matrix"
            );
            expect!(
                dev.GetTransform(D3DTS_VIEW, &mut original_view),
                "unable to backup view matrix"
            );
            expect!(
                dev.GetTransform(D3DTS_PROJECTION, &mut original_proj),
                "unable to backup projection matrix"
            );

            // set our desired state
            expect!(setup_state(dev, viewport), "unable to setup state");

            Self {
                original_state,
                original_world,
                original_view,
                original_proj,
                dev: dev.clone(),
            }
        }
    }
}

impl Drop for DxState {
    fn drop(&mut self) {
        // restore the previous state
        unsafe {
            expect!(
                self.dev.SetTransform(D3DTS_WORLD, &self.original_world),
                "unable to reset world matrix"
            );
            expect!(
                self.dev.SetTransform(D3DTS_VIEW, &self.original_view),
                "unable to reset view matrix"
            );
            expect!(
                self.dev.SetTransform(D3DTS_PROJECTION, &self.original_proj),
                "unable to reset projection matrix"
            );

            expect!(
                self.original_state.Apply(),
                "unable to re-apply captured state"
            );
        }
    }
}

fn setup_state(
    dev: &IDirect3DDevice9,
    viewport: D3DVIEWPORT9,
) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        // general set up
        let backbuffer: IDirect3DSurface9 = dev.GetBackBuffer(0, 0, D3DBACKBUFFER_TYPE_MONO)?;
        dev.SetRenderTarget(0, &backbuffer)?;
        dev.SetViewport(&viewport)?;

        // set up fvf
        dev.SetPixelShader(None)?;
        dev.SetVertexShader(None)?;
        dev.SetFVF(FVF_CUSTOMVERTEX)?;

        // set up matrix
        let l = 0.5;
        let r = viewport.Width as f32 + 0.5;
        let t = 0.5;
        let b = viewport.Height as f32 + 0.5;

        let mat_ident = Matrix4x4 {
            M11: 1.0,
            M22: 1.0,
            M33: 1.0,
            M44: 1.0,
            ..Default::default()
        };

        let mat_proj = Matrix4x4 {
            M11: 2.0 / (r - l),
            M12: 0.0,
            M13: 0.0,
            M14: 0.0,
            M21: 0.0,
            M22: 2.0 / (t - b),
            M23: 0.0,
            M24: 0.0,
            M31: 0.0,
            M32: 0.0,
            M33: 0.5,
            M34: 0.0,
            M41: (l + r) / (l - r),
            M42: (t + b) / (b - t),
            M43: 0.5,
            M44: 1.0,
        };

        dev.SetTransform(D3DTS_WORLD, &mat_ident)?;
        dev.SetTransform(D3DTS_VIEW, &mat_ident)?;
        dev.SetTransform(D3DTS_PROJECTION, &mat_proj)?;

        // set up render state
        dev.SetRenderState(D3DRS_FILLMODE, D3DFILL_SOLID.0 as _)?;
        dev.SetRenderState(D3DRS_SHADEMODE, D3DSHADE_GOURAUD.0 as _)?;
        dev.SetRenderState(D3DRS_ZENABLE, false as _)?;
        dev.SetRenderState(D3DRS_ZWRITEENABLE, false as _)?;
        dev.SetRenderState(D3DRS_ALPHATESTENABLE, false as _)?;
        dev.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as _)?;
        dev.SetRenderState(D3DRS_ALPHABLENDENABLE, true as _)?;
        dev.SetRenderState(D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as _)?;
        dev.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as _)?;
        dev.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as _)?;
        dev.SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, true as _)?;
        dev.SetRenderState(D3DRS_BLENDOPALPHA, D3DBLENDOP_ADD.0 as _)?;
        dev.SetRenderState(D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as _)?;
        dev.SetRenderState(D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as _)?;
        dev.SetRenderState(D3DRS_SCISSORTESTENABLE, true as _)?;
        dev.SetRenderState(D3DRS_FOGENABLE, false as _)?;
        dev.SetRenderState(D3DRS_RANGEFOGENABLE, false as _)?;
        dev.SetRenderState(D3DRS_SPECULARENABLE, false as _)?;
        dev.SetRenderState(D3DRS_STENCILENABLE, false as _)?;
        dev.SetRenderState(D3DRS_CLIPPING, true as _)?;
        dev.SetRenderState(D3DRS_LIGHTING, false as _)?;
        dev.SetRenderState(D3DRS_TEXTUREFACTOR, 0xFFFFFFFF)?;
        dev.SetRenderState(D3DRS_COLORWRITEENABLE, 0xFFFFFFFF)?;
        dev.SetRenderState(D3DRS_SRGBWRITEENABLE, false as _)?;
        dev.SetRenderState(D3DRS_LASTPIXEL, true as _)?;

        // set up texture stages
        dev.SetTextureStageState(0, D3DTSS_COLOROP, D3DTOP_MODULATE.0 as _)?;
        dev.SetTextureStageState(0, D3DTSS_COLORARG0, D3DTA_CURRENT)?;
        dev.SetTextureStageState(0, D3DTSS_COLORARG1, D3DTA_TEXTURE)?;
        dev.SetTextureStageState(0, D3DTSS_COLORARG2, D3DTA_DIFFUSE)?;
        dev.SetTextureStageState(0, D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as _)?;
        dev.SetTextureStageState(0, D3DTSS_ALPHAARG0, D3DTA_CURRENT)?;
        dev.SetTextureStageState(0, D3DTSS_ALPHAARG1, D3DTA_TEXTURE)?;
        dev.SetTextureStageState(0, D3DTSS_ALPHAARG2, D3DTA_DIFFUSE)?;

        dev.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as _)?;
        dev.SetTextureStageState(1, D3DTSS_ALPHAOP, D3DTOP_DISABLE.0 as _)?;

        dev.SetTextureStageState(2, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as _)?;
        dev.SetTextureStageState(2, D3DTSS_ALPHAOP, D3DTOP_DISABLE.0 as _)?;

        // set up sampler
        dev.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as _)?;
        dev.SetSamplerState(0, D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as _)?;
        dev.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as _)?;
        dev.SetSamplerState(0, D3DSAMP_BORDERCOLOR, 0xFFFFFFFF)?;
        dev.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as _)?;
        dev.SetSamplerState(0, D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as _)?;
        dev.SetSamplerState(0, D3DSAMP_ADDRESSW, D3DTADDRESS_CLAMP.0 as _)?;

        Ok(())
    }
}
