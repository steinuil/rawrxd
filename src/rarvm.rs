// const VM_MEMSIZE: usize = 0x40000;
// const VM_MEMMASK: usize = VM_MEMSIZE - 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardFilter {
    None = 0,
    E8,
    E8E9,
    Itanium,
    Rgb,
    Audio,
    Delta,
}
