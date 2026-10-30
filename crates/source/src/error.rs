/// Failures can be classified without parsing diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported source: {0:?}")]
    Unsupported(Unsupported),
    #[error("malformed source: {0:?}")]
    Malformed(Malformed),
    #[error("inspection resource limit: {0:?}")]
    Resource(Resource),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unsupported {
    FileKind,
    Container,
    FormatTag(u16),
    Channels(u16),
    Bits(u16),
    FmtSize(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    Identification,
    Truncated,
    ContainerLength,
    ChunkExtent,
    DuplicateFmt,
    DuplicateData,
    MissingFmt,
    MissingData,
    DataBeforeFmt,
    ZeroChannels,
    ZeroRate,
    BlockAlign,
    ByteRate,
    PartialFrame,
    Arithmetic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    ChunkBudget,
}
