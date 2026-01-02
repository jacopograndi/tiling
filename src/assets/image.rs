use crate::zlib_inflate;

#[derive(Default, Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub raw: Vec<u8>,
}

impl Image {
    // Specification: https://www.w3.org/TR/2003/REC-PNG-20031110/
    pub fn from_png(s: &[u8]) -> Result<Self, String> {
        // Check file signature
        if &s[0..8] != [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
            return Err(format!("Not a png"));
        }

        let mut width: u32 = 0;
        let mut height: u32 = 0;
        let mut image_data_filtered_compressed: Vec<u8> = vec![];
        let mut c: usize = 8;

        let mut transparency = false;

        // Read png chunks
        for _ in 0..10000 {
            let length = u32::from_be_bytes(s[c..c + 4].try_into().unwrap());
            let block_type: &[u8; 4] = s[c + 4..c + 8].try_into().unwrap();
            c += 8;
            match block_type {
                // Header
                b"IHDR" => {
                    assert_eq!(length, 13);
                    width = u32::from_be_bytes(s[c..c + 4].try_into().unwrap());
                    height = u32::from_be_bytes(s[c + 4..c + 8].try_into().unwrap());

                    let bit_depth = s[c + 8];
                    let color_space = s[c + 9];
                    let compression_method = s[c + 10];
                    let filter_method = s[c + 11];
                    let interlacing = s[c + 12];

                    assert_eq!(bit_depth, 8);
                    assert_eq!(compression_method, 0);
                    assert_eq!(filter_method, 0);
                    assert_eq!(interlacing, 0);
                    match color_space {
                        2 => transparency = false,
                        6 => transparency = true,
                        _ => return Err(format!("Color space {color_space} not supported")),
                    };
                }
                // Image data
                b"IDAT" => {
                    image_data_filtered_compressed.extend_from_slice(&s[c..c + length as usize]);
                }
                // Trailer
                b"IEND" => {
                    break;
                }
                _ => {}
            };

            // TODO: check crc
            let _calculated_crc = 0;

            c += length as usize;
            let _crc = u32::from_be_bytes(s[c..c + 4].try_into().unwrap());
            c += 4;
        }

        // Decompress
        let image_data_filtered = zlib_inflate::decompress_zlib(&image_data_filtered_compressed)
            .map_err(|e| format!("{:?}", e))?;

        // Revert filters
        let px_size = if transparency { 4 } else { 3 };
        let row_size = (width * px_size) as i32;
        let mut image_data = vec![0; (width * height * px_size) as usize];

        let xy_idx = |(x, y): (i32, i32)| (x + y * row_size) as usize;
        let bound_check =
            |(x, y): (i32, i32)| (0..row_size).contains(&x) && (0..height as i32).contains(&y);
        let get_at = |xy, image: &Vec<u8>| {
            if bound_check(xy) {
                image[xy_idx(xy)]
            } else {
                0
            }
        };

        for y in 0..height as i32 {
            let j = (y * row_size + y) as usize;
            let filter = image_data_filtered[j];

            match filter {
                0 => {
                    for x in 0..row_size {
                        image_data[xy_idx((x, y))] = image_data_filtered[j + 1 + x as usize];
                    }
                }
                1 => {
                    for x in 0..row_size {
                        let a = get_at((x - px_size as i32, y), &image_data);
                        let recon = image_data_filtered[j + 1 + x as usize];
                        image_data[xy_idx((x, y))] = recon.wrapping_add(a);
                    }
                }
                2 => {
                    for x in 0..row_size {
                        let b = get_at((x, y - 1), &image_data);
                        let recon = image_data_filtered[j + 1 + x as usize];
                        image_data[xy_idx((x, y))] = recon.wrapping_add(b);
                    }
                }
                3 => {
                    for x in 0..row_size {
                        let a = get_at((x - px_size as i32, y), &image_data);
                        let b = get_at((x, y - 1), &image_data);
                        let recon = image_data_filtered[j + 1 + x as usize];
                        let avg = ((a as u32 + b as u32) / 2) as u8;
                        image_data[xy_idx((x, y))] = recon.wrapping_add(avg);
                    }
                }
                4 => {
                    for x in 0..row_size {
                        let a = get_at((x - px_size as i32, y), &image_data);
                        let b = get_at((x, y - 1), &image_data);
                        let c = get_at((x - px_size as i32, y - 1), &image_data);
                        let recon = image_data_filtered[j + 1 + x as usize];
                        let p = a as i32 + b as i32 - c as i32;
                        let pa = (p - a as i32).abs();
                        let pb = (p - b as i32).abs();
                        let pc = (p - c as i32).abs();
                        let pr = if pa <= pb && pa <= pc {
                            a
                        } else if pb <= pc {
                            b
                        } else {
                            c
                        };
                        image_data[xy_idx((x, y))] = recon.wrapping_add(pr);
                    }
                }
                _ => return Err(format!("Unsupported filter {filter} at row {y}")),
            };
        }

        // Add the transparency channel
        let image_data = if !transparency {
            let image_size = (width * height * 4) as usize;
            let mut image_data_with_transparency: Vec<u8> = vec![0; image_size];
            for i in 0..image_size {
                let j = i - i / 4;
                image_data_with_transparency[i] = if i % 4 == 3 { 255 } else { image_data[j] };
            }
            image_data_with_transparency
        } else {
            image_data
        };

        Ok(Self {
            width,
            height,
            raw: image_data,
        })
    }
}
