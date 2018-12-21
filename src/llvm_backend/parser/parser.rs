use std::collections::HashMap;

/* ------------------------------------------------------------------ */

#[derive(Debug, Clone)]
///
pub struct Module {
    pub signatures: Vec<Vec<u8>>,
    // pub imports: Imports,
    // pub exports: Exports,
    // pub functions: HashMap<String, u32>,
    // pub ir: LLVMModule,
}

///
impl Module {
    pub fn new() -> Self {
        Module {
            signatures: Vec::new(),
        }
    }
}

///
struct Exports {
    pub functions: HashMap<String, u32>,
}


///
struct Imports {
    pub functions: HashMap<(String, String), u32>,
}

/* ------------------------------------------------------------------ */

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    BufferEndReached,
    InvalidVaruint1,
    InvalidVaruint7,
    InvalidVarint7,
    InvalidVaruint32,
    InvalidVarint32,
    InvalidVarint64,
    InvalidMagicNumber,
    InvalidVersionNumber,
}

/* ------------------------------------------------------------------ */

#[derive(Debug, Clone)]
/// A single-pass codegen parser.
/// Generates a Module as it deserializes a wasm binary.
pub struct Parser<'a> {
    code: &'a Vec<u8>, // The wasm binary to parse
    cursor: usize, // Used to track the current byte position as the parser advances.
    module: Module, // The generated module
}

/// Contains the implementation of parser
impl <'a> Parser<'a> {
    /// Creates new parser
    pub fn new(code: &'a Vec<u8>) -> Self {
        Parser {
            code,
            cursor: 0, // cursor starts at first byte
            module: Module::new(),
        }
    }

    /// TODO: TEST
    /// Generates the `module` object by calling functions
    /// that parse a wasm module.
    pub fn module(&mut self) {
        println!("\n= module! =");

        // Consume preamble. Panic if it returns an error.
        self.module_preamble().unwrap();
        // Error::BufferEndReached => MalformedWasmModule
        // Error::InvalidMagicNumber => same
        // Error::InvalidVersionNumber => same

        self.module_body().unwrap();
        // Error::BufferEndReached => MalformedWasmModule
        // ...
    }

    /// TODO: TEST
    /// Checks if the following bytes are expected
    /// wasm preamble bytes.
    pub fn module_preamble(&mut self) -> Result<(), Error> {
        println!("\n= module_preamble! =");
        // Consume magic number.
        let magic_no = self.uint32()?;
        // Consume version number.
        let version_no = self.uint32()?;

        println!("magic = 0x{:08x}, version = 0x{:08x}", magic_no, version_no);

        // Magic number must be `\0asm`
        if magic_no != 0x6d736100 {
            return Err(Error::InvalidMagicNumber);
        }

        // Only version 0x01 supported for now.
        if version_no != 0x1 {
            return Err(Error::InvalidVersionNumber);
        }

        Ok(())
    }

    /// TODO: TEST
    pub fn module_body(&mut self) -> Result<(), Error> {
        println!("\n= module_body! =");
        Ok(())
    }

    #[inline]
    /// Gets a byte from the code buffer and (if available)
    /// advances the cursor.
    fn eat_byte(&mut self) -> Option<u8> {
        let index = self.cursor;
        // Check if range is within code buffer bounds
        if index < self.code.len() {
            // Advance the cursor
            self.cursor += 1;
            return Some(self.code[index]);
        }
        None
    }

    /// Gets the next `range` slice of bytes from the code buffer
    /// (if available) and advances the token.
    fn eat_bytes(&mut self, range: usize) -> Option<&[u8]> {
        let start = self.cursor;
        let end = start + range;
        // Check if range is within code buffer bounds
        if end > self.code.len() {
            return None;
        }
        // Advance the cursor
        self.cursor = end;
        Some(&self.code[start..end])
    }

    /// Consumes 1 byte that represents 8-bit unsigned integer
    pub fn uint8(&mut self) -> Result<u8, Error> {
        if let Some(byte) = self.eat_byte() {
            return Ok(byte);
        }
        Err(Error::BufferEndReached)
    }

    /// Consumes 2 bytes that represent 16-bit unsigned integer
    pub fn uint16(&mut self) -> Result<u16, Error> {
        if let Some(bytes) = self.eat_bytes(2) {
            let mut shift = 0;
            let mut result = 0;
            for byte in bytes {
                result |= (*byte as u16) << shift;
                shift += 8;
            }
            return Ok(result);
        }
        Err(Error::BufferEndReached)
    }

    /// Consumes 4 bytes that represent 32-bit unsigned integer
    pub fn uint32(&mut self) -> Result<u32, Error> {
        if let Some(bytes) = self.eat_bytes(4) {
            let mut shift = 0;
            let mut result = 0;
            for byte in bytes {
                result |= (*byte as u32) << shift;
                shift += 8;
            }
            return Ok(result);
        }
        Err(Error::BufferEndReached)
    }

    /// Consumes a byte that represents a 1-bit LEB128 unsigned integer encoding
    fn varuint1(&mut self) -> Result<bool, Error> {
        if let Some(byte) = self.eat_byte() {
            return match byte {
                1 => Ok(true),
                0 => Ok(false),
                _ => Err(Error::InvalidVaruint1),
            };
        }
        // We expect the if statement to return an Ok result. If it doesn't
        // then we are trying to read more than 1 byte, which is invalid for a varuint1
        Err(Error::BufferEndReached)
    }

    /// Consumes a byte that represents a 7-bit LEB128 unsigned integer encoding
    fn varuint7(&mut self) -> Result<u8, Error> {
        if let Some(byte) = self.eat_byte() {
            let mut result = byte;
            // Check if msb is unset.
            if result & 0b1000_0000 != 0 {
                return Err(Error::InvalidVaruint7);
            }
            return Ok(result);
        }
        // We expect the if statement to return an Ok result. If it doesn't
        // then we are trying to read more than 1 byte, which is invalid for a varuint7
        Err(Error::BufferEndReached)
    }

    /// Consumes 1-5 bytes that represent a 32-bit LEB128 unsigned integer encoding
    fn varuint32(&mut self) -> Result<u32, Error> {
        // println!("= varuint32! =");
        let mut result = 0;
        let mut shift = 0;
        while shift < 35 {
            let byte = match self.eat_byte() {
                Some(value) => value,
                None => return Err(Error::BufferEndReached),
            };
            // println!("count = {}, byte = 0b{:08b}", count, byte);
            // Unset the msb and shift by multiples of 7 to the left
            let value = ((byte & !0b10000000) as u32) << shift;
            result |= value;
            // Return if any of the bytes has an unset msb
            if byte & 0b1000_0000 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
        // We expect the loop to terminate early and return an Ok result. If it doesn't
        // then we are trying to read more than 5 bytes, which is invalid for a varuint32
        Err(Error::InvalidVaruint32)
    }

    /// Consumes a byte that represents a 7-bit LEB128 signed integer encoding
    fn varint7(&mut self) -> Result<i8, Error> {
        if let Some(byte) = self.eat_byte() {
            let mut result = byte;
            // Check if msb is unset.
            if result & 0b1000_0000 != 0 {
                return Err(Error::InvalidVarint7);
            }
            // If the 7-bit value is signed, extend the sign.
		    if result & 0b0100_0000 == 0b0100_0000 {
                result |= 0b1000_0000;
            }
            return Ok(result as i8);
        }
        // We expect the if statement to return an Ok result. If it doesn't
        // then we are trying to read more than 1 byte, which is invalid for a varint7
        Err(Error::BufferEndReached)
    }

    /// Consumes 1-5 bytes that represent a 32-bit LEB128 signed integer encoding
    fn varint32(&mut self) -> Result<i32, Error> {
        // println!("= varint32! =");
        let mut result = 0;
        let mut shift = 0;
        // Can consume at most 5 bytes
        while shift < 35 { // (shift = 0, 7, 14 .. 35)
            let byte = match self.eat_byte() {
                Some(value) => value,
                None => return Err(Error::BufferEndReached),
            };
            // println!("count = {}, byte = 0b{:08b}", count, byte);
            // Unset the msb and shift by multiples of 7 to the left
            let value = ((byte & !0b10000000) as i32) << shift;
            result |= value;
            // Return if any of the bytes has an unset msb
            if byte & 0b1000_0000 == 0 {
                // Extend sign if sign bit is set. We don't bother when we are on the 5th byte
                // (hence shift < 28) because it gives an 32-bit value, so no need for sign
                // extension there
                if shift < 28 && byte & 0b0100_0000 != 0 {
                    result |= -1 << (7 + shift); // -1 == 0xff_ff_ff_ff
                }
                return Ok(result);
            }
            shift += 7;
        }
        // We expect the loop to terminate early and return an Ok result. If it doesn't
        // then we are trying to read more than 5 bytes, which is invalid for a varint32
        Err(Error::InvalidVarint32)
    }

    /// TODO: TEST
    /// Consumes 1-9 bytes that represent a 64-bit LEB128 signed integer encoding
    fn varint64(&mut self) -> Result<i64, Error> {
        // println!("= varint64! =");
        let mut result = 0;
        let mut shift = 0;
        // Can consume at most 9 bytes
        while shift < 63 { // (shift = 0, 7, 14 .. 56)
            let byte = match self.eat_byte() {
                Some(value) => value,
                None => return Err(Error::BufferEndReached),
            };
            // println!("count = {}, byte = 0b{:08b}", count, byte);
            // Unset the msb and shift by multiples of 7 to the left
            let value = ((byte & !0b10000000) as i64) << shift;
            result |= value;
            // Return if any of the bytes has an unset msb
            if byte & 0b1000_0000 == 0 {
                // Extend sign if sign bit is set. We don't bother when we are on the 9th byte
                // (hence shift < 56) because it gives an 64-bit value, so no need for sign
                // extension there
                if shift < 56 && byte & 0b0100_0000 != 0 {
                    result |= -1 << (7 + shift); // -1 == 0xff_ff_ff_ff
                }
                return Ok(result);
            }
            shift += 7;
        }
        // We expect the loop to terminate early and return an Ok result. If it doesn't
        // then we are trying to read more than 5 bytes, which is invalid for a varint32
        Err(Error::InvalidVarint64)
    }
}

// pub fn compile(source: Vec<u8>) -> Module {
// }

#[cfg(test)]
mod parser_tests {
    use super::Parser;
    use super::Error;

    #[test]
    fn eat_byte_can_consume_next_byte_if_available() {
        let code = vec![0x6d];
        let mut parser = Parser::new(&code);
        let result = parser.eat_byte().unwrap();
        assert_eq!(result, 0x6d);
    }

    #[test]
    fn eat_byte_can_consume_just_the_next_byte_if_available() {
        let code = vec![0x01, 0x00];
        let mut parser = Parser::new(&code);
        let result = parser.eat_byte().unwrap();
        assert_eq!(result, 0x1);
    }

    #[test]
    fn eat_byte_can_consume_just_the_next_byte_if_available_2() {
        let code = vec![0x01, 0x5f];
        let mut parser = Parser::new(&code);
        // Consume first byte.
        let result = parser.eat_byte();
        // Then consume the next byte.
        let result = parser.eat_byte().unwrap();
        assert_eq!(result, 0x5f);
    }

    #[test]
    fn eat_byte_cannot_consume_next_byte_if_not_available() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.eat_byte();
        assert!(result.is_none());
    }

    #[test]
    fn eat_bytes_can_consume_next_specified_bytes_if_available() {
        let code = vec![0x00, 0x61, 0x73, 0x6d];
        let mut parser = Parser::new(&code);
        let result = parser.eat_bytes(4).unwrap();
        assert_eq!(result, &[0x00, 0x61, 0x73, 0x6d]);
    }

    #[test]
    fn eat_bytes_can_consume_next_specified_bytes_if_available_2() {
        let code = vec![0x00, 0x61, 0x73, 0x6d, 0x1];
        let mut parser = Parser::new(&code);
        let result = parser.eat_bytes(5).unwrap();
        assert_eq!(result, &[0x00, 0x61, 0x73, 0x6d, 0x1]);
    }

    #[test]
    fn eat_bytes_can_consume_next_specified_bytes_if_available_3() {
        let code = vec![0x01, 0x10, 0x73, 0x6d, 0x09, 0xff, 0x5e];
        let mut parser = Parser::new(&code);
        // Consume 4 bytes first.
        let result = parser.eat_bytes(4);
        // Then consume the next 3 bytes.
        let result = parser.eat_bytes(3).unwrap();
        assert_eq!(result, &[0x09, 0xff, 0x5e]);
    }

    #[test]
    fn eat_bytes_can_consume_just_the_next_specified_bytes_if_available() {
        let code = vec![0x01, 0x00, 0x73, 0x00, 0x1];
        let mut parser = Parser::new(&code);
        let result = parser.eat_bytes(1).unwrap();
        assert_eq!(result, &[0x1]);
    }

    #[test]
    fn eat_bytes_cannot_consume_next_specified_bytes_if_not_available() {
        let code = vec![0x01, 0x00, 0x00];
        let mut parser = Parser::new(&code);
        let result = parser.eat_bytes(4);
        assert!(result.is_none());
    }

    #[test]
    fn eat_bytes_cannot_consume_next_specified_bytes_if_not_available_2() {
        let code = vec![0x01, 0x10, 0x73, 0x6d, 0x09, 0xff, 0x5e];
        let mut parser = Parser::new(&code);
        // Consume 5 bytes first.
        let result = parser.eat_bytes(5);
        // Then consume the next 3 bytes.
        let result = parser.eat_bytes(3);
        assert!(result.is_none());
    }

    #[test]
    fn uint8_can_consume_next_byte_if_available() {
        let code = vec![0x22];
        let mut parser = Parser::new(&code);
        let result = parser.uint8().unwrap();
        assert_eq!(result, 0x22);
    }

    #[test]
    fn uint8_can_consume_just_the_next_byte_if_available() {
        let code = vec![0x00, 0x61, 0x73, 0x6d];
        let mut parser = Parser::new(&code);
        let result = parser.uint8().unwrap();
        assert_eq!(result, 0x00);
    }

    #[test]
    fn uint8_cannot_consume_next_byte_if_not_available_2() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.uint8().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn uint16_can_consume_next_2_bytes_if_available() {
        let code = vec![0x00, 0x61];
        let mut parser = Parser::new(&code);
        let result = parser.uint16().unwrap();
        assert_eq!(result, 0x6100);
    }

    #[test]
    fn uint16_can_consume_just_the_next_2_bytes_if_available() {
        let code = vec![0x01, 0x00, 0x73, 0x6d];
        let mut parser = Parser::new(&code);
        let result = parser.uint16().unwrap();
        assert_eq!(result, 0x1);
    }

    #[test]
    fn uint16_cannot_consume_next_2_bytes_if_not_available() {
        let code = vec![0x01];
        let mut parser = Parser::new(&code);
        let result = parser.uint16().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn uint16_cannot_consume_next_2_bytes_if_not_available_2() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.uint16().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn uint32_can_consume_next_4_bytes_if_available() {
        let code = vec![0x00, 0x61, 0x73, 0x6d];
        let mut parser = Parser::new(&code);
        let result = parser.uint32().unwrap();
        assert_eq!(result, 0x6d736100);
    }

    #[test]
    fn uint32_can_consume_just_the_next_4_bytes_if_available() {
        let code = vec![0x01, 0x00, 0x00, 0x00, 0x1];
        let mut parser = Parser::new(&code);
        let result = parser.uint32().unwrap();
        assert_eq!(result, 0x1);
    }

    #[test]
    fn uint32_cannot_consume_next_4_bytes_if_not_available() {
        let code = vec![0x01, 0x00, 0x00];
        let mut parser = Parser::new(&code);
        let result = parser.uint32().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn uint32_cannot_consume_next_4_bytes_if_not_available_2() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.uint32().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn varuint7_can_consume_next_byte_if_available_and_valid() {
        let code = vec![0b0111_0100];
        let mut parser = Parser::new(&code);
        let result = parser.varuint7().unwrap();
        assert_eq!(result, 0b0111_0100);
    }

    #[test]
    fn varuint7_can_consume_next_byte_if_available_and_valid_2() {
        let code = vec![0b0100_0000];
        let mut parser = Parser::new(&code);
        let result = parser.varuint7().unwrap();
        assert_eq!(result, 0b0100_0000);
    }

    #[test]
    fn varuint7_cannot_consume_next_byte_if_not_available() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.varuint7().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn varuint7_cannot_consume_next_byte_if_not_valid_varuint7() {
        let code = vec![0b1000_0000];
        let mut parser = Parser::new(&code);
        let result = parser.varuint7().unwrap_err();
        assert_eq!(result, Error::InvalidVaruint7);
    }

    #[test]
    fn varuint32_can_consume_next_bytes_if_available_and_valid() {
        let code = vec![0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_0000, 0b0000_1000];
        let mut parser = Parser::new(&code);
        let result = parser.varuint32().unwrap();
        assert_eq!(result, 0b1000_0000_0000_0000_0000_0000_0000_0000);
    }

    #[test]
    fn varuint32_can_consume_next_bytes_if_available_and_valid_2() {
        let code = vec![0b1111_1111, 0b1111_1111, 0b0000_0011, 0b1010_1010];
        let mut parser = Parser::new(&code);
        let result = parser.varuint32().unwrap();
        assert_eq!(result, 0b0000_0000_0000_0000_1111_1111_1111_1111);
    }

    #[test]
    fn varuint32_cannot_consume_next_bytes_if_not_available() {
        let code = vec![0b1000_0000];
        let mut parser = Parser::new(&code);
        let result = parser.varuint32().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn varuint32_cannot_consume_next_bytes_if_not_valid_varuint32() {
        let code = vec![0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_1000];
        let mut parser = Parser::new(&code);
        let result = parser.varuint32().unwrap_err();
        assert_eq!(result, Error::InvalidVaruint32);
    }

    #[test]
    fn varint7_can_consume_next_byte_if_available_and_valid() {
        let code = vec![0x7f, 0x00, 0x00];
        let mut parser = Parser::new(&code);
        let result = parser.varint7().unwrap();
        assert_eq!(result, -0x1);
    }

    #[test]
    fn varint7_can_consume_next_byte_if_available_and_valid_2() {
        let code = vec![0x60];
        let mut parser = Parser::new(&code);
        let result = parser.varint7().unwrap();
        assert_eq!(result, -0x20);
    }

    #[test]
    fn varint7_cannot_consume_next_byte_if_not_available() {
        let code = vec![];
        let mut parser = Parser::new(&code);
        let result = parser.varint7().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn varint7_cannot_consume_next_byte_if_not_valid_varint7() {
        let code = vec![0b1000_0000];
        let mut parser = Parser::new(&code);
        let result = parser.varint7().unwrap_err();
        assert_eq!(result, Error::InvalidVarint7);
    }

    #[test]
    fn varint32_can_consume_next_bytes_if_available_and_valid() {
        let code = vec![0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_0000, 0b0111_1000,];
        let mut parser = Parser::new(&code);
        let result = parser.varint32().unwrap();
        assert_eq!(result, -2147483648);
    }

    #[test]
    fn varint32_can_consume_next_bytes_if_available_and_valid_2() {
        let code = vec![0b1110_0000, 0b1010_1011, 0b1110_1101, 0b0111_1101, 0b0011_0110];
        let mut parser = Parser::new(&code);
        let result = parser.varint32().unwrap();
        assert_eq!(result, -4_500_000);
    }

    #[test]
    fn varint32_cannot_consume_next_bytes_if_not_available() {
        let code = vec![0b1000_0000, 0b1010_1011, 0b1110_1101];
        let mut parser = Parser::new(&code);
        let result = parser.varint32().unwrap_err();
        assert_eq!(result, Error::BufferEndReached);
    }

    #[test]
    fn varint32_cannot_consume_next_bytes_if_not_valid_varint32() {
        let code = vec![0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_0000, 0b1000_1000];
        let mut parser = Parser::new(&code);
        let result = parser.varint32().unwrap_err();
        assert_eq!(result, Error::InvalidVarint32);
    }
}

