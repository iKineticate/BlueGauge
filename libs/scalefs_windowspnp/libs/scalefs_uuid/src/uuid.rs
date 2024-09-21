// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use std::str::FromStr;

// NOTE: we break the Uuid's data fields down into four data fields (and annotate the corresponding component labels from RFC 4122; note that RFC 4122 is not a complete modern UUID spec and that we have combined the last three fields into an 8-octet sequence to match convention)
#[derive(PartialEq, Eq, Hash)]
pub struct Uuid {
    pub data1: u32,     // time-low
    pub data2: u16,     // time-mid
    pub data3: u16,     // time-high-and-version
    pub data4: [u8; 8], // clock-seq-and-reserved | clock-seq-low | node[6] 
}
impl Uuid {
    pub fn from_u128(uuid_as_u128: u128) -> Self {
        Self {
            data1: ((uuid_as_u128 >> 96) & 0xFFFF_FFFF) as u32,
            data2: ((uuid_as_u128 >> 80) & 0xFFFF) as u16,
            data3: ((uuid_as_u128 >> 64) & 0xFFFF) as u16,
            data4: (((uuid_as_u128 >> 0) & 0xFFFF_FFFF_FFFF_FFFF) as u64).to_be_bytes(),
        }
    }

    pub fn as_u128(&self) -> u128 {
        ((self.data1 as u128) << 96) |
        ((self.data2 as u128) << 80) |
        ((self.data3 as u128) << 64) |
        ((u64::from_be_bytes(self.data4) as u128) << 0)
    }

    // see: rfc 4122
    pub fn is_nil_uuid(&self) -> bool {
        self.as_u128() == 0
    }
}

//

impl From<windows::core::GUID> for Uuid {
    fn from(value: windows::core::GUID) -> Self {
        Self {
            data1: value.data1,
            data2: value.data2,
            data3: value.data3,
            data4: value.data4,
        }
    }
}

impl From<Uuid> for windows::core::GUID {
    fn from(value: Uuid) -> Self {
        Self {
            data1: value.data1,
            data2: value.data2,
            data3: value.data3,
            data4: value.data4,
        }
    }
}

impl From<windows_sys::core::GUID> for Uuid {
    fn from(value: windows_sys::core::GUID) -> Self {
        Self {
            data1: value.data1,
            data2: value.data2,
            data3: value.data3,
            data4: value.data4,
        }
    }
}

impl From<Uuid> for windows_sys::core::GUID {
    fn from(value: Uuid) -> Self {
        Self {
            data1: value.data1,
            data2: value.data2,
            data3: value.data3,
            data4: value.data4,
        }
    }
}

//

// NOTE: this should implement debug output as well as to_string
impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data1_as_hex = format!("{:08x}", self.data1);
        let data2_as_hex = format!("{:04x}", self.data2);
        let data3_as_hex = format!("{:04x}", self.data3);
        let upper_data4_as_hex = format!("{:02x}{:02x}", self.data4[0], self.data4[1]);
        let lower_data4_as_hex = format!("{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}", self.data4[2], self.data4[3], self.data4[4], self.data4[5], self.data4[6], self.data4[7]);

        write!(f, "{}-{}-{}-{}-{}", data1_as_hex, data2_as_hex, data3_as_hex, upper_data4_as_hex, lower_data4_as_hex)
    }
}

//

#[derive(Debug, PartialEq, Eq)]
pub struct ParseUuidError;

impl FromStr for Uuid {
    type Err = ParseUuidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars().collect::<Vec<char>>();

        if chars.len() == 0 {
            // an empty string is not a valid parse string
            // NOTE: we may want to consider returning a nil uuid (all-zero) instead
            return Err(ParseUuidError { });
        }

        // if present, remove leading and trailing curly braces
        if chars.first().unwrap() == &'{' {
            if chars.len() == 1 {
                // a leading curly brace by itself is invalid
                return Err(ParseUuidError { });
            }

            if chars.last().unwrap() != &'}' {
                // a leading curly brace must be matched with a trailing curly brace
                return Err(ParseUuidError { });
            }

            // remove the leading and trailing curly braces
            _ = chars.remove(0);
            _ = chars.remove(chars.len() - 1);

            if chars.len() == 0 {
                // no contents exist within the curly braces
                return Err(ParseUuidError { });
            }
        }

        let capture_leading_u32_from_str_radix = |chars: &mut Vec<char>| -> Result<u32, ParseUuidError> {
            let hex_digit_len = 8;

            if chars.len() < hex_digit_len { return Err(ParseUuidError { }); } // not enough content
            if chars[0..hex_digit_len].iter().all(|ch| ch.is_ascii_alphanumeric()) == false {
                // digits are not hex digits
                return Err(ParseUuidError { });
            }
            let u32_value = match u32::from_str_radix(&chars[0..hex_digit_len].to_vec().iter().collect::<String>(), 16) {
                Ok(value) => value,
                Err(_) => return Err(ParseUuidError { }),
            };
            _ = chars.drain(0..hex_digit_len);

            Ok(u32_value)
        };

        let capture_leading_u16_from_str_radix = |chars: &mut Vec<char>| -> Result<u16, ParseUuidError> {
            let hex_digit_len = 4;

            if chars.len() < hex_digit_len { return Err(ParseUuidError { }); } // not enough content
            if chars[0..hex_digit_len].iter().all(|ch| ch.is_ascii_alphanumeric()) == false {
                // digits are not hex digits
                return Err(ParseUuidError { });
            }
            let u16_value = match u16::from_str_radix(&chars[0..hex_digit_len].to_vec().iter().collect::<String>(), 16) {
                Ok(value) => value,
                Err(_) => return Err(ParseUuidError { }),
            };
            _ = chars.drain(0..hex_digit_len);

            Ok(u16_value)
        };

        let capture_leading_u8_from_str_radix = |chars: &mut Vec<char>| -> Result<u8, ParseUuidError> {
            let hex_digit_len = 2;

            if chars.len() < hex_digit_len { return Err(ParseUuidError { }); } // not enough content
            if chars[0..hex_digit_len].iter().all(|ch| ch.is_ascii_alphanumeric()) == false {
                // digits are not hex digits
                return Err(ParseUuidError { });
            }
            let u8_value = match u8::from_str_radix(&chars[0..hex_digit_len].to_vec().iter().collect::<String>(), 16) {
                Ok(value) => value,
                Err(_) => return Err(ParseUuidError { }),
            };
            _ = chars.drain(0..hex_digit_len);

            Ok(u8_value)
        };

        let remove_leading_hyphen_closure = |chars: &mut Vec<char>| -> Result<(), ParseUuidError> {
            if chars.len() < 1 { return Err(ParseUuidError { }); } // not enough content
            if chars.first().unwrap() != &'-' {
                // character is not a hyphen
                return Err(ParseUuidError { });
            }
            _ = chars.remove(0);

            Ok(())
        };

        // parse the uuid
        //
        // capture data1 (u32)
        let data1 = match capture_leading_u32_from_str_radix(&mut chars) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        //
        // capture hyphen
        if let Err(error) = remove_leading_hyphen_closure(&mut chars) {
            return Err(error);
        }
        // capture data2 (u16)
        let data2 = match capture_leading_u16_from_str_radix(&mut chars) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        //
        // capture hyphen
        if let Err(error) = remove_leading_hyphen_closure(&mut chars) {
            return Err(error);
        }
        // capture data3 (u16)
        let data3 = match capture_leading_u16_from_str_radix(&mut chars) {
            Ok(value) => value,
            Err(error) => return Err(error),
        };
        //
        // capture hyphen
        if let Err(error) = remove_leading_hyphen_closure(&mut chars) {
            return Err(error);
        }
        //
        // capture first two octets of data4
        let mut data4_as_vec = Vec::<u8>::with_capacity(8);
        for _ in 0..2 {
            let data4_element = match capture_leading_u8_from_str_radix(&mut chars) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            data4_as_vec.push(data4_element);
        }
        //
        // capture hyphen (i.e. the hyphen that breaks up the 2 and 6 octets of data4)
        if let Err(error) = remove_leading_hyphen_closure(&mut chars) {
            return Err(error);
        }
        //
        // capture remaining six octets of data4
        for _ in 2..8 {
            let data4_element = match capture_leading_u8_from_str_radix(&mut chars) {
                Ok(value) => value,
                Err(error) => return Err(error),
            };
            data4_as_vec.push(data4_element);
        }
        let data4: [u8; 8] = data4_as_vec.try_into().unwrap();

        if chars.len() > 0 {
            // all chars should have been consumed at this point
            return Err(ParseUuidError { });
        }

        Ok(Self {
            data1,
            data2,
            data3,
            data4,
        })
    }
}
