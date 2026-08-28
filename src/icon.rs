const IDLE_BODY: [u8; 3] = [154, 158, 164];
const IDLE_OUTLINE: [u8; 3] = [83, 87, 94];
const ACTIVE_BODY: [u8; 3] = [224, 42, 42];
const ACTIVE_OUTLINE: [u8; 3] = [139, 24, 24];
const WHEEL: [u8; 3] = [75, 78, 84];

/// Draws a small, antialiased mouse silhouette suitable for window and tray icons.
/// The returned pixels are in RGBA order.
pub fn mouse_rgba(size: u32, active: bool) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }

    let (body, outline) = if active {
        (ACTIVE_BODY, ACTIVE_OUTLINE)
    } else {
        (IDLE_BODY, IDLE_OUTLINE)
    };
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    let samples = 4u32;

    for y in 0..size {
        for x in 0..size {
            let mut red = 0u32;
            let mut green = 0u32;
            let mut blue = 0u32;
            let mut covered = 0u32;

            for sample_y in 0..samples {
                for sample_x in 0..samples {
                    let px = (x as f32 + (sample_x as f32 + 0.5) / samples as f32)
                        / size as f32;
                    let py = (y as f32 + (sample_y as f32 + 0.5) / samples as f32)
                        / size as f32;

                    let outer = inside_rounded_rect(px, py, 0.25, 0.04, 0.75, 0.96, 0.23);
                    if !outer {
                        continue;
                    }

                    let wheel = inside_rounded_rect(px, py, 0.465, 0.20, 0.535, 0.39, 0.035);
                    let inner = inside_rounded_rect(px, py, 0.28, 0.075, 0.72, 0.925, 0.20);
                    let color = if wheel {
                        WHEEL
                    } else if inner {
                        body
                    } else {
                        outline
                    };
                    red += u32::from(color[0]);
                    green += u32::from(color[1]);
                    blue += u32::from(color[2]);
                    covered += 1;
                }
            }

            if covered == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.extend_from_slice(&[
                    (red / covered) as u8,
                    (green / covered) as u8,
                    (blue / covered) as u8,
                    ((covered * 255) / (samples * samples)) as u8,
                ]);
            }
        }
    }

    rgba
}

fn inside_rounded_rect(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    radius: f32,
) -> bool {
    let nearest_x = x.clamp(left + radius, right - radius);
    let nearest_y = y.clamp(top + radius, bottom - radius);
    let dx = x - nearest_x;
    let dy = y - nearest_y;
    dx * dx + dy * dy <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::mouse_rgba;

    #[test]
    fn mouse_icon_has_transparent_corners_and_an_opaque_body() {
        let icon = mouse_rgba(32, false);
        assert_eq!(&icon[..4], &[0, 0, 0, 0]);
        let center = ((16 * 32 + 16) * 4) as usize;
        assert_eq!(icon[center + 3], 255);
    }

    #[test]
    fn active_mouse_icon_is_red() {
        let icon = mouse_rgba(32, true);
        assert!(icon
            .chunks_exact(4)
            .any(|pixel| pixel[3] == 255 && pixel[0] > 200 && pixel[1] < 80));
    }
}
