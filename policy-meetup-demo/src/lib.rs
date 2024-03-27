// Copyright 2023 Salesforce, Inc. All rights reserved.
mod generated;

use anyhow::{anyhow, Result};

use pdk::hl::*;
use pdk::logger;
use serde_json::json;

use crate::generated::config::Config;

async fn request_filter (request_state: RequestState, config: &Config, client: HttpClient) ->Flow<()>{

    let header_state = request_state.into_headers_state().await;
    let header_handler = header_state.handler();

    //Verifica se Header configurado em "Header" esta presente
    if header_handler.header(&config.header.as_str()).is_none(){
        logger::error!("Header {} nao esta presente", config.header.as_str());
        let message = format!("Header {} obrigatorio", config.header.as_str());
        return error_message(&message, 403);
    }

    //Verifica se Header "senha" esta presente
    let path: String;
    if let Some(senha) = header_handler.header("senha"){
        path = "/api/valida_senha?senha=".to_owned() + senha.as_str();
        logger::info!("Path - {}", path)
    }
    else{
        logger::error!("Header senha nao esta presente");
        let message = "Header senha obrigatorio";
        return error_message(message, 403);
    }

    let body_state = header_state.into_body_state().await;
    let body_handler = body_state.handler();
    let body = body_handler.body();

    //Verifica se o Body esta presente
    if body.is_empty(){
        let message = "Body obrigatorio";
        return error_message(message, 403);
    };

    //Valida o formato do body
    let json_body = match serde_json::from_slice::<serde_json::Value>(&body) {
        Ok(json) => {
            json
        }
        Err(_) =>{
            let message = "Body - Invalid Json format";
            return error_message(message, 403);
        }        
    };

    //Verifica se o valor que esta no "tagBody" esta presente
    if !json_body.as_object().unwrap().contains_key(&config.tag_body){
        let message = format! ("Body - Cmapo {} obrigatorio", config.tag_body.as_str());
        return error_message(&message, 403);
    };

    //Verifica se o campo "cliente" esta presente
    let cliente = match json_body.get("cliente").and_then(|v|v.as_str()) {
        Some(value) => value,
        None => {
            logger::error!("Campo cliente nao encontrado no Body");
            let message = "Campo cliente Obrigatorio no Body";
            return error_message(message, 403);
        }        
    };

    // Monta o body para a request
    let request_body = serde_json::to_vec(
        &json!(
            { 
                "cliente": cliente
            }
        )
    ).unwrap_or_default();

    //Faz a chamada ao serviço externo
    logger::info!("Fazendo chamada externa");
    let result = request_service(config, client, request_body, path).await;

    //Verifica se a requisição ao serviço foi bem sucedida
    match result {
        Ok(Some(response_body)) => {
            //Converter o corpo da resposta para Json
            let json_response_body = serde_json::from_slice::<serde_json::Value>(&response_body);
            //Verifica se foi possivel converter em Json e se o campo de validação esta presente e tem o valor esperado
            match json_response_body {
                Ok(json) => {
                    if let Some(token) = json["token"].as_str(){
                        if !token.is_empty(){
                            logger::info!("Senha Valida, token Criado")
                        }
                        else{
                            logger::error!("Senha Invalida, token vazio");
                            let message = "Senha Invalida";
                            return error_message(message, 401);
                        }
                    }
                    else{
                        logger::error!("token nao encontrado no corpo da resposta");
                        let message = String::from_utf8_lossy(&response_body);
                        return error_message(&message, 500);    
                    }
                }
                Err(_) => {
                    //Falha ao analisar o corpoda resposta
                    logger::error!("Erro ao analisar o Resposta da Validação");
                    let message = "Erro ao analisar o Resposta da Validação";
                    return error_message(message, 500);
                }
            }
        }
        Ok(None) => {
            //A Resposta da validação esta vazia
            logger::error!("Resposta do serviço esta vazia");
            let message = "Resposta da Validação vazia - Tente novamente mais tarde";
            return error_message(message, 500);
        }
        Err(_) => {
            //Falha na requisição da validação
            logger::error!("Erro ao chamar a validação");
            let message = "Erro na validação - verifique os dados enviados";
            return error_message(message, 500);
        }
    }

    Flow::Continue(())
}

async fn response_filter (response_state: ResponseState){
    let header_state = response_state.into_headers_state().await;
    let header_hander = header_state.handler();

    header_hander.add_header("Powered-by", "PDK");
}

async fn request_service (config: &Config, cliente: HttpClient, request_body: Vec<u8>, path: String) -> Result<Option<Vec<u8>>, anyhow::Error>{
    let response = cliente
        .request(&config.service_value)
        .path(&path)
        .headers(vec![("Content-Type", "application/json")])
        .body(&request_body)
        .post()
        .await
        .map_err(|err | anyhow!("Error sending request: {:?}", err))?;

    let body = response.body().to_vec();
    Ok(Some(body))
}

fn error_message(message: &str, status_code: u32) -> Flow<()> {
    let headers: Vec<(String, String)> = vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Powered-By".to_string(), "pdk".to_string()),
    ];

    let error = format!("{{\"error\": \"{}\"}}", message);

    Flow::Break(Response::new(status_code)
        .with_headers(headers)
        .with_body(error)) 
}


#[entrypoint]
async fn configure(launcher: Launcher, Configuration(bytes): Configuration) -> Result<()> {
    let config: Config = serde_json::from_slice(&bytes).map_err(|err| {
        anyhow!(
            "Failed to parse configuration '{}'. Cause: {}",
            String::from_utf8_lossy(&bytes),
            err
        )
    })?;
    let filter = on_request(|rs, client| request_filter(rs, &config, client))
    .on_response(|rs| response_filter(rs));
    launcher.launch(filter).await?;
    Ok(())
}
