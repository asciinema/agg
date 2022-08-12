use std::str::FromStr;

use anyhow::bail;
use rgb::RGB8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub background: RGB8,
    pub foreground: RGB8,
    palette: [RGB8; 16],
}

fn parse_hex_triplet(triplet: &str) -> anyhow::Result<RGB8> {
    if triplet.len() < 6 || triplet.len() > 6 {
        bail!("{} is not a hex triplet", triplet);
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
            bail!("expected 10 or 18 hex triplets, got {}", colors.len());
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

#[cfg(test)]
mod tests {
    use super::Theme;
    use rgb::RGB8;

    #[test]
    fn parse_invalid() {
        assert!("".parse::<Theme>().is_err());

        assert!("foo".parse::<Theme>().is_err());

        assert!("000000,111111,222222,333333,444444"
            .parse::<Theme>()
            .is_err());

        assert!(
            "xxxxxx,111111,222222,333333,444444,555555,666666,777777,888888,999999"
                .parse::<Theme>()
                .is_err()
        );
    }

    #[test]
    fn parse_8_color_palette() {
        let result = "bbbbbb,ffffff,000000,111111,222222,333333,444444,555555,666666,777777"
            .parse::<Theme>();

        assert!(result.is_ok());

        let theme = result.unwrap();

        assert_eq!(
            theme.background,
            RGB8 {
                r: 0xbb,
                g: 0xbb,
                b: 0xbb
            }
        );

        assert_eq!(
            theme.foreground,
            RGB8 {
                r: 0xff,
                g: 0xff,
                b: 0xff
            }
        );

        assert_eq!(
            theme.palette,
            [
                RGB8 {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00
                },
                RGB8 {
                    r: 0x11,
                    g: 0x11,
                    b: 0x11
                },
                RGB8 {
                    r: 0x22,
                    g: 0x22,
                    b: 0x22
                },
                RGB8 {
                    r: 0x33,
                    g: 0x33,
                    b: 0x33
                },
                RGB8 {
                    r: 0x44,
                    g: 0x44,
                    b: 0x44
                },
                RGB8 {
                    r: 0x55,
                    g: 0x55,
                    b: 0x55
                },
                RGB8 {
                    r: 0x66,
                    g: 0x66,
                    b: 0x66
                },
                RGB8 {
                    r: 0x77,
                    g: 0x77,
                    b: 0x77
                },
                RGB8 {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00
                },
                RGB8 {
                    r: 0x11,
                    g: 0x11,
                    b: 0x11
                },
                RGB8 {
                    r: 0x22,
                    g: 0x22,
                    b: 0x22
                },
                RGB8 {
                    r: 0x33,
                    g: 0x33,
                    b: 0x33
                },
                RGB8 {
                    r: 0x44,
                    g: 0x44,
                    b: 0x44
                },
                RGB8 {
                    r: 0x55,
                    g: 0x55,
                    b: 0x55
                },
                RGB8 {
                    r: 0x66,
                    g: 0x66,
                    b: 0x66
                },
                RGB8 {
                    r: 0x77,
                    g: 0x77,
                    b: 0x77
                },
            ]
        );
    }

    #[test]
    fn parse_16_color_palette() {
        let result = "bbbbbb,ffffff,000000,111111,222222,333333,444444,555555,666666,777777,888888,999999,aaaaaa,bbbbbb,cccccc,dddddd,eeeeee,ffffff".parse::<Theme>();

        assert!(result.is_ok());

        let theme = result.unwrap();

        assert_eq!(
            theme.background,
            RGB8 {
                r: 0xbb,
                g: 0xbb,
                b: 0xbb
            }
        );

        assert_eq!(
            theme.foreground,
            RGB8 {
                r: 0xff,
                g: 0xff,
                b: 0xff
            }
        );

        assert_eq!(
            theme.palette,
            [
                RGB8 {
                    r: 0x00,
                    g: 0x00,
                    b: 0x00
                },
                RGB8 {
                    r: 0x11,
                    g: 0x11,
                    b: 0x11
                },
                RGB8 {
                    r: 0x22,
                    g: 0x22,
                    b: 0x22
                },
                RGB8 {
                    r: 0x33,
                    g: 0x33,
                    b: 0x33
                },
                RGB8 {
                    r: 0x44,
                    g: 0x44,
                    b: 0x44
                },
                RGB8 {
                    r: 0x55,
                    g: 0x55,
                    b: 0x55
                },
                RGB8 {
                    r: 0x66,
                    g: 0x66,
                    b: 0x66
                },
                RGB8 {
                    r: 0x77,
                    g: 0x77,
                    b: 0x77
                },
                RGB8 {
                    r: 0x88,
                    g: 0x88,
                    b: 0x88
                },
                RGB8 {
                    r: 0x99,
                    g: 0x99,
                    b: 0x99
                },
                RGB8 {
                    r: 0xaa,
                    g: 0xaa,
                    b: 0xaa
                },
                RGB8 {
                    r: 0xbb,
                    g: 0xbb,
                    b: 0xbb
                },
                RGB8 {
                    r: 0xcc,
                    g: 0xcc,
                    b: 0xcc
                },
                RGB8 {
                    r: 0xdd,
                    g: 0xdd,
                    b: 0xdd
                },
                RGB8 {
                    r: 0xee,
                    g: 0xee,
                    b: 0xee
                },
                RGB8 {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff
                },
            ]
        );
    }
}
