//! Scale bounded raw RGB before its only PNG encoding; never decode an original.
use super::*;
pub fn encode_visual_rgb(size: [u32; 2], rgb: &[u8], max_edge: u32) -> Result<(Vec<u8>, [u32; 2])> {
    rwi_core::VisualOptions {
        max_edge,
        ..Default::default()
    }
    .validate()?;
    Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
    .checked()?;
    if rgb.len() != size[0] as usize * size[1] as usize * 3 {
        return Err(fault(
            "encoding_source_layout",
            "Visual input must be contiguous, bounded three-byte RGB pixels.",
        ));
    }
    let edge = size[0].max(size[1]);
    let output = if edge <= max_edge {
        size
    } else {
        size.map(|v| ((u64::from(v) * u64::from(max_edge)) / u64::from(edge)).max(1) as u32)
    };
    if output == size {
        return Ok((
            encode_rgb_limited(size, rgb, rwi_core::MAX_VISUAL_PNG_BYTES)?,
            size,
        ));
    }
    let sampled = sampling::area_rgb(
        size,
        rgb,
        Rect {
            x: 0,
            y: 0,
            width: size[0],
            height: size[1],
        },
        output,
    )?;
    Ok((
        encode_rgb_limited(output, &sampled, rwi_core::MAX_VISUAL_PNG_BYTES)?,
        output,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_rgb_area_resize_preserves_linear_gradient_means() {
        let size = [256, 128];
        let mut rgb = Vec::new();
        for y in 0..size[1] {
            for x in 0..size[0] {
                rgb.extend_from_slice(&[x as u8, y as u8, 37]);
            }
        }
        let (png, output) = encode_visual_rgb(size, &rgb, 64).unwrap();
        let decoded = decode_original(&png).unwrap();
        assert_eq!(output, [64, 32]);
        assert_eq!(decoded.size, output);
        assert_eq!(&decoded.rgb[..3], &[2, 2, 37]);
        assert_eq!(&decoded.rgb[decoded.rgb.len() - 3..], &[254, 126, 37]);
    }
    #[test]
    fn small_images_and_thin_sources_are_never_upscaled() {
        let rgb = [1, 2, 3, 4, 5, 6];
        let (png, output) = encode_visual_rgb([2, 1], &rgb, 2048).unwrap();
        assert_eq!(output, [2, 1]);
        assert_eq!(decode_original(&png).unwrap().rgb, rgb);
        let (_, output) = encode_visual_rgb([1, 4096], &vec![7; 4096 * 3], 64).unwrap();
        assert_eq!(output, [1, 64]);
    }
    #[test]
    fn source_dimensions_layout_and_output_byte_limits_are_separate() {
        assert!(encode_visual_rgb([4096, 4096], &[], 1024).is_err());
        assert!(encode_visual_rgb([1, 1], &[1, 2], 64).is_err());
        assert!(encode_visual_rgb([1, 1], &[1, 2, 3], 63).is_err());
        assert_eq!(
            encode_rgb_limited([1, 1], &[1, 2, 3], 16).unwrap_err().code,
            "png_size_limit"
        );
        let op = rwi_core::operation::Registry::default()
            .begin(None)
            .unwrap();
        op.cancel();
        assert_eq!(
            rwi_core::operation::run(&op, || encode_visual_rgb([1, 1], &[1, 2, 3], 64))
                .unwrap_err()
                .code,
            "operation_cancelled"
        );
    }
}
