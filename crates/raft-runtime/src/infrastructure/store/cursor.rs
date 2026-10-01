use super::*;

pub(super) struct CursorReader<'a>(pub(super) &'a [u8]);

impl<'a> CursorReader<'a> {
    pub(super) fn read_slice(&mut self, len: usize) -> io::Result<&'a [u8]> {
        if self.0.len() < len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected eof reading byte payload",
            ));
        }
        let (data, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(data)
    }

    pub(super) fn skip_bytes(&mut self, len: usize) -> io::Result<()> {
        self.read_slice(len).map(|_| ())
    }

    pub(super) fn read_u8(&mut self) -> io::Result<u8> {
        if self.0.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected eof reading u8",
            ));
        }
        let b = self.0[0];
        self.0 = &self.0[1..];
        Ok(b)
    }

    pub(super) fn read_u64(&mut self) -> io::Result<u64> {
        if self.0.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected eof reading u64",
            ));
        }
        let (num_bytes, rest) = self.0.split_at(8);
        self.0 = rest;
        Ok(u64::from_le_bytes(num_bytes.try_into().unwrap()))
    }

    pub(super) fn read_u32(&mut self) -> io::Result<u32> {
        if self.0.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected eof reading u32",
            ));
        }
        let (num_bytes, rest) = self.0.split_at(4);
        self.0 = rest;
        Ok(u32::from_le_bytes(num_bytes.try_into().unwrap()))
    }

    pub(super) fn read_bytes(&mut self, len: usize) -> io::Result<Vec<u8>> {
        Ok(self.read_slice(len)?.to_vec())
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
