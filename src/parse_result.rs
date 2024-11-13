#[derive(Debug)]
pub enum ParseResult<T, E> {
    Success(T),
    Garbage(E),
}
