use std::collections::HashMap;

use egui::{ImageData, TextureId, TexturesDelta};
use windows::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Direct3D9::{
        IDirect3DDevice9, IDirect3DTexture9, D3DFMT_A8R8G8B8, D3DLOCKED_RECT, D3DLOCK_DISCARD,
        D3DLOCK_READONLY, D3DPOOL_DEFAULT, D3DPOOL_SYSTEMMEM, D3DUSAGE_DYNAMIC,
    },
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TextureColor {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    pub a: u8,
}

struct ManagedTexture {
    handle: Option<IDirect3DTexture9>,
    pixels: Vec<TextureColor>,
    size: [usize; 2],
}

pub struct TextureManager {
    textures: HashMap<TextureId, ManagedTexture>,
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }
}

impl TextureManager {
    pub fn process_set_deltas(&mut self, dev: &IDirect3DDevice9, delta: &TexturesDelta) {
        delta.set.iter().for_each(|(tid, delta)| {
            // check if this texture already exists
            if self.textures.get(tid).is_some() {
                if delta.is_whole() {
                    // update the entire texture
                    self.update_texture_whole(dev, tid, &delta.image);
                } else {
                    // update part of the texture
                    self.update_texture_area(
                        dev,
                        tid,
                        &delta.image,
                        expect!(delta.pos, "unable to extract delta position"),
                    );
                }
            } else {
                // create new texture
                self.create_new_texture(dev, tid, &delta.image)
            }
        });
    }

    pub fn process_free_deltas(&mut self, delta: &TexturesDelta) {
        delta.free.iter().for_each(|tid| {
            self.free(tid);
        });
    }

    pub fn get_by_id(&self, id: TextureId) -> &IDirect3DTexture9 {
        expect!(
            &expect!(self.textures.get(&id), "unable to retrieve texture")
                .handle
                .as_ref(),
            "unable to retrieve texture handle"
        )
    }

    pub fn deallocate_textures(&mut self) {
        self.textures.iter_mut().for_each(|(_tid, texture)| {
            texture.handle = None;
        });
    }

    pub fn reallocate_textures(&mut self, dev: &IDirect3DDevice9) {
        self.textures.iter_mut().for_each(|(_tid, texture)| {
            let handle = new_texture_from_buffer(dev, &texture.pixels, texture.size);

            texture.handle = Some(handle);
        });
    }
}

impl TextureManager {
    fn free(&mut self, tid: &TextureId) -> bool {
        self.textures.remove(tid).is_some()
    }

    fn create_new_texture(
        &mut self,
        dev: &IDirect3DDevice9,
        tid: &TextureId,
        img_data: &ImageData,
    ) {
        let pixels = pixels_from_imagedata(img_data);
        let size = img_data.size();

        let handle = new_texture_from_buffer(dev, &pixels, size);

        self.textures.insert(
            *tid,
            ManagedTexture {
                handle: Some(handle),
                pixels,
                size,
            },
        );
    }

    fn update_texture_area(
        &mut self,
        dev: &IDirect3DDevice9,
        tid: &TextureId,
        img_data: &ImageData,
        pos: [usize; 2],
    ) {
        let x = pos[0];
        let y = pos[1];
        let w = img_data.width();
        let h = img_data.height();

        let pixels = pixels_from_imagedata(img_data);

        let temp_tex = create_temporary_texture(dev, &pixels, [w, h]);

        unsafe {
            let texture = expect!(
                self.textures.get(tid),
                "unable to get texture to delta patch"
            );

            let src_surface = expect!(temp_tex.GetSurfaceLevel(0), "unable to get tex surface");

            let dst_surface = expect!(
                expect!(texture.handle.as_ref(), "unable to get texture handle").GetSurfaceLevel(0),
                "unable to get tex surface"
            );

            expect!(
                dev.UpdateSurface(
                    &src_surface,
                    &RECT {
                        left: 0 as _,
                        right: w as _,
                        top: 0 as _,
                        bottom: h as _,
                    },
                    &dst_surface,
                    &POINT {
                        x: x as _,
                        y: y as _,
                    },
                ),
                "unable to update surface"
            );
        }
    }

    fn update_texture_whole(
        &mut self,
        dev: &IDirect3DDevice9,
        tid: &TextureId,
        img_data: &ImageData,
    ) {
        let texture = expect!(self.textures.get_mut(tid), "unable to get texture");
        let size = img_data.size();

        let pixels = pixels_from_imagedata(img_data);

        if size != texture.size {
            // size mismatch, recreate texture
            // free texture
            self.free(tid);

            // create a new texture with new data
            let handle = new_texture_from_buffer(dev, &pixels, size);

            // insert new texture under same key
            self.textures.insert(
                *tid,
                ManagedTexture {
                    handle: Some(handle),
                    pixels,
                    size,
                },
            );
        } else {
            // perfectly normal update operation
            let temp_tex = create_temporary_texture(dev, &pixels, size);

            unsafe {
                expect!(
                    expect!(texture.handle.as_ref(), "unable to get texture handle").AddDirtyRect(
                        &RECT {
                            left: 0,
                            top: 0,
                            right: size[0] as _,
                            bottom: size[1] as _
                        }
                    ),
                    "unable to dirty texture"
                );

                expect!(
                    dev.UpdateTexture(
                        &temp_tex,
                        expect!(texture.handle.as_ref(), "unable to get texture handle")
                    ),
                    "unable to update texture"
                );
            }

            texture.pixels = pixels;
        }
    }
}

fn pixels_from_imagedata(img_data: &ImageData) -> Vec<TextureColor> {
    match img_data {
        ImageData::Font(f) => f
            .srgba_pixels(None)
            .map(|c| {
                let cols = c.to_array();
                TextureColor {
                    r: cols[0],
                    g: cols[1],
                    b: cols[2],
                    a: cols[3],
                }
            })
            .collect(),
        ImageData::Color(x) => x
            .pixels
            .iter()
            .map(|c| {
                let cols = c.to_array();
                TextureColor {
                    r: cols[0],
                    g: cols[1],
                    b: cols[2],
                    a: cols[3],
                }
            })
            .collect(),
    }
}

fn create_temporary_texture(
    dev: &IDirect3DDevice9,
    buf: &[TextureColor],
    size: [usize; 2],
) -> IDirect3DTexture9 {
    unsafe {
        let mut temp_texture: Option<IDirect3DTexture9> = None;

        expect!(
            dev.CreateTexture(
                size[0] as _,
                size[1] as _,
                1,
                D3DUSAGE_DYNAMIC as _,
                D3DFMT_A8R8G8B8,
                D3DPOOL_SYSTEMMEM,
                &mut temp_texture,
                std::ptr::null_mut()
            ),
            "unable to create temporary texture"
        );

        let temp_texture = expect!(temp_texture, "unable to create temporary texture");

        let mut locked_rect = D3DLOCKED_RECT::default();

        expect!(
            temp_texture.LockRect(
                0,
                &mut locked_rect,
                std::ptr::null_mut(),
                D3DLOCK_DISCARD as u32 | D3DLOCK_READONLY as u32
            ),
            "unable to lock temporary texture"
        );

        std::slice::from_raw_parts_mut(locked_rect.pBits as *mut TextureColor, size[0] * size[1])
            .copy_from_slice(buf);

        expect!(
            temp_texture.UnlockRect(0),
            "unable to unlock temporary texture"
        );

        temp_texture
    }
}

fn new_texture_from_buffer(
    dev: &IDirect3DDevice9,
    buf: &[TextureColor],
    size: [usize; 2],
) -> IDirect3DTexture9 {
    let temp_tex = create_temporary_texture(dev, buf, size);
    let mut texture: Option<IDirect3DTexture9> = None;

    unsafe {
        expect!(
            dev.CreateTexture(
                size[0] as _,
                size[1] as _,
                1,
                D3DUSAGE_DYNAMIC as _,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            ),
            "unable to create texture"
        );

        let texture = expect!(texture, "unable to create texture");

        expect!(
            dev.UpdateTexture(&temp_tex, &texture),
            "unable to upload texture"
        );

        texture
    }
}
