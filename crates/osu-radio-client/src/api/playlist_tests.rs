use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn read_request(stream: &mut tokio::net::TcpStream) -> (String, Vec<u8>) {
    let mut headers = Vec::new();
    while !headers.ends_with(b"\r\n\r\n") {
        headers.push(stream.read_u8().await.unwrap());
    }
    let headers = String::from_utf8(headers).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|length| length.parse::<usize>().ok())
        })
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    (headers, body)
}

#[tokio::test]
async fn playlist_cover_http_methods_preserve_png_bytes_and_summary_revisions() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
    let png = b"\x89PNG\r\n\x1a\n\x00\xffbinary";
    let server = tokio::spawn(async move {
        for method in ["GET", "PUT", "DELETE"] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let (headers, body) = read_request(&mut stream).await;
            assert!(headers.starts_with(&format!("{method} /api/playlists/7/cover HTTP/1.1\r\n")));
            let response_body = if method == "PUT" {
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("content-type: image/png\r\n")
                );
                assert_eq!(body, png);
                br#"{"id":7,"name":"Evening","item_count":3,"cover_beatmap_id":100,"custom_cover_revision":2}"#.to_vec()
            } else if method == "GET" {
                png.to_vec()
            } else {
                Vec::new()
            };
            let status = if method == "DELETE" {
                "204 No Content"
            } else {
                "200 OK"
            };
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response_body.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&response_body).await.unwrap();
        }
    });
    assert_eq!(
        api.playlist_cover(7).await.unwrap().as_deref(),
        Some(png.as_slice())
    );
    let summary = api.set_playlist_cover(7, png.to_vec()).await.unwrap();
    assert_eq!(summary.item_count, 3);
    assert_eq!(summary.custom_cover_revision, Some(2));
    api.reset_playlist_cover(7).await.unwrap();
    server.await.unwrap();
    assert!(matches!(
        api.set_playlist_cover(7, vec![0; 2 * 1024 * 1024 + 1])
            .await,
        Err(ApiError::Protocol(_))
    ));
}

#[tokio::test]
async fn playlist_cover_download_rejects_oversized_headers_and_streams_and_handles_missing_images()
{
    let too_large = 2 * 1024 * 1024 + 1;
    for response in [
        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        format!("HTTP/1.1 200 OK\r\nContent-Length: {too_large}\r\n\r\n").into_bytes(),
        {
            let mut response = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{too_large:x}\r\n").into_bytes();
            response.resize(response.len() + too_large, 0);
            response.extend_from_slice(b"\r\n0\r\n\r\n");
            response
        },
    ] {
        let missing = response.starts_with(b"HTTP/1.1 404");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            read_request(&mut stream).await;
            let _ = stream.write_all(&response).await;
        });
        let result = api.playlist_cover(7).await;
        if missing {
            assert_eq!(result.unwrap(), None);
        } else {
            assert!(matches!(result, Err(ApiError::Protocol(_))));
        }
        server.await.unwrap();
    }
}
