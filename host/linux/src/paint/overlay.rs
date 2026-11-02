use super::*;

/// A desktop menu or confirmation painted over the page.
pub(crate) struct TopSurface {
    pub rect: Rect4,
    pub pixels: Arc<Pixmap>,
    pub modal: bool,
    pub scrim: [u8; 4],
    pub shadow: Option<(Arc<Pixmap>, Rect4)>,
}

impl Painter {
    pub(crate) fn set_top_surface(&mut self, surface: Option<TopSurface>) {
        if self.top_surface.is_some() || surface.is_some() {
            // Opening and dismissing a menu changes pixels outside text damage.
            self.damage.pixels = None;
        }
        self.top_surface = surface;
    }

    pub(super) fn paint_top_surface(&mut self, viewport: (f32, f32)) {
        if let Some(surface) = &self.top_surface {
            if surface.modal {
                self.backend.fill(
                    &Shape::rect((0., 0., viewport.0, viewport.1)),
                    surface.scrim,
                    Transform::identity(),
                );
            }
            if let Some((pixels, rect)) = &surface.shadow {
                self.backend.surface_image(pixels.clone(), *rect);
            }
            self.backend
                .surface_image(surface.pixels.clone(), surface.rect);
        }
    }
}
