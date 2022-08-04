use std::str::FromStr;

use anyhow::anyhow;
use rgb::RGB8;

#[derive(Clone, Debug)]
pub struct Theme {
    pub background: RGB8,
    pub foreground: RGB8,
    palette: [RGB8; 16],
}

fn parse_hex_triplet(triplet: &str) -> anyhow::Result<RGB8> {
    if triplet.len() < 6 || triplet.len() > 6 {
        return Err(anyhow!("{} is not a hex triplet", triplet));
    }

    let r = u8::from_str_radix(&triplet[0..2], 16)?;
    let g = u8::from_str_radix(&triplet[2..4], 16)?;
    let b = u8::from_str_radix(&triplet[4..6], 16)?;

    Ok(RGB8::new(r, g, b))
}

impl FromStr for Theme {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut palette = [RGB8::default(); 16];

        let colors = s
            .split(',')
            .filter(|s| !s.is_empty())
            .map(parse_hex_triplet)
            .collect::<anyhow::Result<Vec<RGB8>>>()?;

        if colors.len() != 10 && colors.len() != 18 {
            return Err(anyhow!(
                "expected 10 or 18 hex triplets, got {}",
                colors.len()
            ));
        }

        let background = colors[0];
        let foreground = colors[1];

        for (i, color) in colors.into_iter().skip(2).cycle().take(16).enumerate() {
            palette[i] = color;
        }

        Ok(Self {
            background,
            foreground,
            palette,
        })
    }
}

impl Theme {
    pub fn color(&self, color: u8) -> RGB8 {
        match color {
            0..=15 => self.palette[color as usize],

            16..=231 => {
                let n = color - 16;
                let mut r = ((n / 36) % 6) * 40;
                let mut g = ((n / 6) % 6) * 40;
                let mut b = (n % 6) * 40;

                if r > 0 {
                    r += 55;
                }

                if g > 0 {
                    g += 55;
                }

                if b > 0 {
                    b += 55;
                }

                RGB8::new(r, g, b)
            }

            232.. => {
                let v = 8 + 10 * (color - 232);

                RGB8::new(v, v, v)
            }
        }
    }
}
