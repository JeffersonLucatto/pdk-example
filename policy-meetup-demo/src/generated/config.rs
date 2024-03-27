use serde::Deserialize;
#[derive(Deserialize, Clone, Debug)]
pub struct Config {
    #[serde(alias = "ambiente")]
    pub ambiente: String,
    #[serde(alias = "header")]
    pub header: String,
    #[serde(
        alias = "serviceValue",
        deserialize_with = "pdk::serde::deserialize_service"
    )]
    pub service_value: pdk::hl::Service,
    #[serde(alias = "tagBody")]
    pub tag_body: String,
    #[serde(alias = "validar")]
    pub validar: Option<bool>,
}
#[pdk::hl::entrypoint_flex]
fn init(abi: &dyn pdk::flex_abi::api::FlexAbi) -> Result<(), anyhow::Error> {
    let config: Config = serde_json::from_slice(abi.get_configuration())
        .map_err(|err| {
            anyhow::anyhow!(
                "Failed to parse configuration '{}'. Cause: {}",
                String::from_utf8_lossy(abi.get_configuration()), err
            )
        })?;
    abi.service_create(config.service_value)?;
    Ok(())
}
