use image::imageops::{FilterType, grayscale, resize};
use image::{ImageBuffer, Luma, Rgb};

const IMG_SCALE: u32 = 8;

pub fn compute_dhash(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> [u8; 8] {
    let signature = to_grayscale_signature(img);

    let mut hash = 0u64;
    let mut bit_position = 0;

    for y in 0..IMG_SCALE {
        for x in 0..IMG_SCALE {
            let current_pixel = signature.get_pixel(x, y)[0];
            let next_pixel = signature.get_pixel(x + 1, y)[0];

            if current_pixel < next_pixel {
                hash |= 1 << bit_position;
            }

            bit_position += 1;
        }
    }

    hash.to_be_bytes()
}

fn to_grayscale_signature(img: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let gray_image = grayscale(img);

    resize(&gray_image, IMG_SCALE + 1, IMG_SCALE, FilterType::Triangle)
}

pub fn hamming_distance(left: &[u8], right: &[u8]) -> u32 {
    left.iter()
        .zip(right.iter())
        .map(|(a, b)| (a ^ b).count_ones())
        .sum()
}
