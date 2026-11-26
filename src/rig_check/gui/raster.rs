//! A small software rasterizer for headless mockup screenshots (#248).
//!
//! Runs egui without a window, tessellates the frame and fills the
//! triangles on the CPU, so mockups can be produced on any machine with no
//! display server, GPU or device access.

use eframe::egui::{self, Color32, Pos2, Rect, TextureId, epaint};
use std::collections::HashMap;

struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
}

impl Texture {
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let x = (u * self.width as f32 - 0.5).clamp(0.0, (self.width - 1) as f32);
        let y = (v * self.height as f32 - 0.5).clamp(0.0, (self.height - 1) as f32);
        let (x0, y0) = (x.floor() as usize, y.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let at = |x: usize, y: usize| {
            let c = self.pixels[y * self.width + x];
            [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]
        };
        let (a, b, c, d) = (at(x0, y0), at(x1, y0), at(x0, y1), at(x1, y1));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

fn apply(textures: &mut HashMap<TextureId, Texture>, delta: &egui::TexturesDelta) {
    for (id, image_delta) in &delta.set {
        let epaint::ImageData::Color(image) = &image_delta.image;
        let [w, h] = image.size;
        match image_delta.pos {
            None => {
                textures.insert(
                    *id,
                    Texture {
                        width: w,
                        height: h,
                        pixels: image.pixels.clone(),
                    },
                );
            }
            Some([px, py]) => {
                if let Some(texture) = textures.get_mut(id) {
                    for y in 0..h {
                        for x in 0..w {
                            if px + x < texture.width && py + y < texture.height {
                                texture.pixels[(py + y) * texture.width + px + x] =
                                    image.pixels[y * w + x];
                            }
                        }
                    }
                }
            }
        }
    }
    for id in &delta.free {
        textures.remove(id);
    }
}

/// Draw `ui` for a few frames (so layout settles) and return the last frame
/// as RGBA pixels, `width` x `height` at one pixel per point.
pub fn render(width: u32, height: u32, ui: impl FnMut(&egui::Context)) -> image::RgbaImage {
    render_scaled(width, height, 1, ui)
}

/// Draw at a native scale while keeping width and height in logical points.
pub(super) fn render_scaled(
    width: u32,
    height: u32,
    scale: u32,
    mut ui: impl FnMut(&egui::Context),
) -> image::RgbaImage {
    let ctx = egui::Context::default();
    super::apply_theme(&ctx);
    let pixels_per_point = scale as f32;
    let mut textures = HashMap::new();
    let mut last = None;
    for _ in 0..4 {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(width as f32, height as f32),
            )),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root viewport")
            .native_pixels_per_point = Some(pixels_per_point);
        let output = ctx.run(input, &mut ui);
        apply(&mut textures, &output.textures_delta);
        last = Some(output);
    }
    let output = last.expect("ran at least once");
    let primitives = ctx.tessellate(output.shapes, pixels_per_point);
    let (width, height) = (width * scale, height * scale);
    let mut canvas = vec![[0.0f32; 4]; (width * height) as usize];
    for primitive in primitives {
        let epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        let Some(texture) = textures.get(&mesh.texture_id) else {
            continue;
        };
        let to_pixels = |pos: Pos2| pos * pixels_per_point;
        let clip = Rect::from_min_max(
            to_pixels(primitive.clip_rect.min),
            to_pixels(primitive.clip_rect.max),
        );
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v = tri.map(|index| {
                let mut vertex = mesh.vertices[index as usize];
                vertex.pos = to_pixels(vertex.pos);
                vertex
            });
            fill(&mut canvas, width, height, clip, &v, texture);
        }
    }
    let mut image = image::RgbaImage::new(width, height);
    for (pixel, value) in image.pixels_mut().zip(canvas) {
        *pixel = image::Rgba([
            value[0].round().clamp(0.0, 255.0) as u8,
            value[1].round().clamp(0.0, 255.0) as u8,
            value[2].round().clamp(0.0, 255.0) as u8,
            255,
        ]);
    }
    image
}

fn fill(
    canvas: &mut [[f32; 4]],
    width: u32,
    height: u32,
    clip: Rect,
    v: &[epaint::Vertex; 3],
    texture: &Texture,
) {
    let (a, b, c) = (v[0].pos, v[1].pos, v[2].pos);
    let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    if area.abs() < 1e-6 {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).max(clip.min.x).max(0.0).floor() as i32;
    let max_x =
        a.x.max(b.x)
            .max(c.x)
            .min(clip.max.x)
            .min(width as f32)
            .ceil() as i32;
    let min_y = a.y.min(b.y).min(c.y).max(clip.min.y).max(0.0).floor() as i32;
    let max_y =
        a.y.max(b.y)
            .max(c.y)
            .min(clip.max.y)
            .min(height as f32)
            .ceil() as i32;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            let w0 = ((b.x - p.x) * (c.y - p.y) - (b.y - p.y) * (c.x - p.x)) / area;
            let w1 = ((c.x - p.x) * (a.y - p.y) - (c.y - p.y) * (a.x - p.x)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
            let t = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
            let colour = |i: usize| {
                let c = |vertex: &epaint::Vertex| vertex.color.to_array()[i] as f32;
                w0 * c(&v[0]) + w1 * c(&v[1]) + w2 * c(&v[2])
            };
            let tex = texture.sample(u, t);
            let src: [f32; 4] = std::array::from_fn(|i| tex[i] * colour(i) / 255.0);
            let dst = &mut canvas[(y as u32 * width + x as u32) as usize];
            let keep = 1.0 - src[3] / 255.0;
            for i in 0..4 {
                dst[i] = src[i] + dst[i] * keep;
            }
        }
    }
}
