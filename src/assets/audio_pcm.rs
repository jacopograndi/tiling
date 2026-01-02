use crate::ByteDecoder;

#[derive(Debug)]
pub struct AudioPcm {
    pub samples: Vec<f32>,
    // TODO: expose the frequency too
}

impl AudioPcm {
    pub fn from_wav(s: &[u8]) -> Result<Self, String> {
        let mut decoder = ByteDecoder::new(s);
        decoder.check_bytes(b"RIFF")?;
        let _file_size_minus_8 = decoder.decode_u32_le();
        decoder.check_bytes(b"WAVE")?;

        decoder.check_bytes(b"fmt ")?;
        let _chunk_size_minus_8 = decoder.decode_u32_le();
        let audio_format = decoder.decode_u16_le();
        let interleaved_channels = decoder.decode_u16_le();
        let frequency = decoder.decode_u32_le();
        let _byte_per_sec = decoder.decode_u32_le();
        let _byte_per_chunk = decoder.decode_u16_le();
        let bits_per_sample = decoder.decode_u16_le();

        // ignore optional list chunk
        if let Ok(()) = decoder.check_bytes(b"LIST") {
            let list_chunk_size_minus_8 = decoder.decode_u32_le();
            decoder.cursor += list_chunk_size_minus_8 as usize;
        }

        decoder.check_bytes(b"data")?;
        let data_size = decoder.decode_u32_le();

        let mut samples: Vec<f32> = vec![];
        if bits_per_sample == 16 && audio_format == 1 {
            for _i in 0..(data_size / (bits_per_sample as u32 / 8)) {
                let sample = decoder.decode_i16_le();
                samples.push((sample as f32) / 32768.);
            }
        }

        if interleaved_channels == 1 {
            // mono to stereo
            samples = samples
                .into_iter()
                .flat_map(|sample| [sample, sample])
                .collect();
        }

        // TODO: move this resampling downstream when i know to what frequency to resample
        // stupid nearest-neighbor resampler taken from quad_snd
        if frequency != 48000 {
            let mut new_length =
                ((48000 as f32 / frequency as f32) * samples.len() as f32) as usize;

            // `new_length` must be an even number
            new_length -= new_length % 2;

            let mut resampled = vec![0.0; new_length];

            for (n, sample) in resampled.chunks_exact_mut(2).enumerate() {
                let ix = 2 * ((n as f32 / new_length as f32) * samples.len() as f32) as usize;
                sample[0] = samples[ix];
                sample[1] = samples[ix + 1];
            }
            return Ok(Self { samples: resampled });
        }

        Ok(Self { samples })
    }
}
