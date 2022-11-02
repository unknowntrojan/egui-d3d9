use windows::Win32::Graphics::Direct3D9::{IDirect3DDevice9, IDirect3DStateBlock9, D3DSBT_ALL};

pub struct StateBackup {
    state: IDirect3DStateBlock9,
}

impl StateBackup {
    pub fn new(dev: &IDirect3DDevice9) -> Self {
        unsafe {
            let state = expect!(
                dev.CreateStateBlock(D3DSBT_ALL),
                "unable to create state block"
            );
            expect!(state.Capture(), "unable to capture dx9 state block");

            Self { state }
        }
    }
}

impl Drop for StateBackup {
    fn drop(&mut self) {
        unsafe {
            expect!(self.state.Apply(), "unable to re-apply state block");
        }
    }
}
