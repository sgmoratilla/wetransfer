use std::fs;
use std::path::Path;

use responses::CreateTransferResponse;
use requests::CreateTransferRequest;
use responses::WeTransferError;
use requests::FileRequest;
use serde::de::DeserializeOwned;
use serde::Serialize;
use reqwest::{Response, Client};
use std::error::Error;


#[derive(Debug)]
pub struct TransferService {
    pub jwt: String,
    pub app_token : String,
    http_client: Client,
}

#[cfg(not(test))]
const TRANSFERS_URL: &'static str = "https://dev.wetransfer.com/v2/transfers";
#[cfg(test)]
const TRANSFERS_URL: &'static str = mockito::SERVER_URL;


pub struct Transfer {
    pub status: String,
    pub message: String,
    pub url: String
}

impl TransferService {
    pub fn new<S: Into<String>+ToString>(jwt: S, app_token: S) -> TransferService {
        TransferService {
            jwt: jwt.to_string(),
            app_token: app_token.to_string(),
            http_client: Client::new(),
        }
    }

    // pub fn create<S: Into<String>+ToString>(&self, message: S, paths: [&str]) -> Result<Transfer, WeTransferError> {
    //     let transfer = self.create_transfer_request(message, paths);
    //     for (file, index) in transfer.files.iter().enumerate() {
    //         for part in (0..=file.multipart.parts) {
    //             let s3_url = self.upload_url(file.id, part);
    //             self.upload_chunk(paths[index], file.multipart.chunk_size);
    //         }
    //         self.mark_as_complete(file.id, file.multipart.parts);
    //     }
    //     self.finalize(transfer.id);
    //     Transfer { }
    // }


    pub fn create_transfer_request<S: Into<String>+ToString>(&self, message: S, paths: Vec<S>) -> Result<CreateTransferResponse, WeTransferError> {
        let files = paths.into_iter().map(|path_str: S| {
            // Compiler suggested to put this expression under
            // a let variable.
            let path_str_ref = &path_str.to_string();
            let path = Path::new(path_str_ref);
            self.extract_file_info(&path)
        }).collect();

        match files {
            Ok(file_requests) => {
                let payload = CreateTransferRequest {
                    message: message.to_string(),
                    files: file_requests
                };

                self.perform_request::<CreateTransferRequest, CreateTransferResponse>(TRANSFERS_URL, payload)
            },
            Err(transfer_error) => Err(transfer_error)
        }
    }

    fn extract_file_info(&self, path: &Path) -> Result<FileRequest, WeTransferError> {
        match fs::metadata(path) {
            Ok(io) => Ok(FileRequest {
                // Safe to unwrap path after a successful read
                name: path.file_name().unwrap().to_str().unwrap().to_string(),
                size: io.len()
            }),
            Err(error) => Err(WeTransferError {
                status: 0,
                message: error.description().to_string(),
            })
        }
    }

    fn perform_request<T: Serialize, U: DeserializeOwned>(&self, url: &str, payload: T) -> Result<U, WeTransferError> {
        let result = self.http_client
            .post(url)
            .header("x-api-key", self.app_token.to_string())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.jwt))
            .json(&payload).send();
        match result {
            Ok(mut response) => {
                if response.status().is_success() {
                    match response.json::<U>() {
                        Ok(final_response) => Ok(final_response),
                        Err(error) => Err(WeTransferError { status: 0, message: error.description().to_string() })
                    }
                } else {
                    let parsed_result = response.json::<WeTransferError>();
                    match parsed_result {
                        Ok(mut wetransfer_error) => {
                            wetransfer_error.status = response.status().as_u16();
                            Err(wetransfer_error)
                        },
                        Err(_) => Err(WeTransferError{
                            status: 0,
                            message: String::from("Error while parsing a WeTransfer error response. Contact mainteners.")
                        }) 
                    }
                    // Err(WeTransferError { status: response.status().as_u16(), message: response.description().to_string() })
                }
            },
            Err(error) => Err(WeTransferError {
                status: 0,
                message: error.description().to_string()
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;
    use std::fs;

    #[test]
    fn it_creates_transfers() {
        let body = fs::read_to_string(Path::new("src/support/create_transfer_request.json")).expect("Fixtures:");
        println!("{}", mockito::SERVER_URL);
        let _m = mock("POST", "/")
          .with_status(201)
          .match_header("Authorization", "Bearer jwt-token")
          .match_header("x-api-key", "1234")
          .with_body(body)
          .create();

        let service = TransferService::new("jwt-token", "1234");
        let transfer_request = service.create_transfer_request("foo", vec!["Cargo.toml"]).unwrap();
        assert!(transfer_request.success);
        assert_eq!(transfer_request.id, "32a6ef6003f1429be0cf1674dd8fbdef20181019143517");
        assert_eq!(transfer_request.message, "foo");
        assert_eq!(transfer_request.state, "uploading");
        assert_eq!(transfer_request.url, None);
        assert_eq!(transfer_request.files[0].multipart.part_numbers, 1);
        assert_eq!(transfer_request.files[0].multipart.chunk_size, 212);
        assert_eq!(transfer_request.files[0].file_type, "file");
        assert_eq!(transfer_request.files[0].name, "Cargo.toml");
        assert_eq!(transfer_request.files[0].id, "c964caf6c54343f3b6e9610cb4ac5ea220181019143517");
    }
}
