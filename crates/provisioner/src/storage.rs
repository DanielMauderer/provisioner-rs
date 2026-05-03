pub trait Storage {
    type Error;

    fn load(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
    fn store(&mut self, data: &[u8]) -> Result<(), Self::Error>;
}
