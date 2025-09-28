#[cfg(feature = "oas30")]
pub mod oas30;
#[cfg(feature = "oas31")]
pub mod oas31;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum OASMajorVersion {
    #[cfg(feature = "oas30")]
    OAS30,
    #[cfg(feature = "oas31")]
    OAS31,
}
