use des::cipher::{BlockEncrypt, KeyInit};
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub(super) fn password_key(password: &[u8]) -> io::Result<[u8; 8]> {
    if password.is_empty() || password.len() > 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "EXACT_VNC_PASSWORD must contain one to eight bytes",
        ));
    }
    let mut key = [0u8; 8];
    for (out, byte) in key.iter_mut().zip(password) {
        *out = byte.reverse_bits();
    }
    Ok(key)
}

fn response(key: &[u8; 8], challenge: [u8; 16]) -> [u8; 16] {
    let cipher = des::Des::new(key.into());
    let mut answer = challenge;
    for chunk in answer.chunks_exact_mut(8) {
        cipher.encrypt_block(chunk.into());
    }
    answer
}

/// RFB security type 2: https://www.rfc-editor.org/rfc/rfc6143#section-7.2.2
pub(super) fn handshake(stream: &mut TcpStream, key: &[u8; 8]) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    stream.write_all(b"RFB 003.008\n")?;
    let mut version = [0u8; 12];
    stream.read_exact(&mut version)?;
    let minor = match &version {
        b"RFB 003.003\n" => 3,
        b"RFB 003.007\n" => 7,
        b"RFB 003.008\n" => 8,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported RFB version",
            ))
        }
    };
    if minor == 3 {
        stream.write_all(&2u32.to_be_bytes())?;
    } else {
        stream.write_all(&[1, 2])?;
        let mut chosen = [0];
        stream.read_exact(&mut chosen)?;
        if chosen != [2] {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "VNC authentication required",
            ));
        }
    }
    let mut challenge = [0u8; 16];
    getrandom::fill(&mut challenge).map_err(|error| io::Error::other(error.to_string()))?;
    stream.write_all(&challenge)?;
    let mut answer = [0u8; 16];
    stream.read_exact(&mut answer)?;
    let difference = answer
        .iter()
        .zip(response(key, challenge))
        .fold(0, |d, (a, b)| d | (a ^ b));
    if difference != 0 {
        stream.write_all(&1u32.to_be_bytes())?;
        if minor == 8 {
            let reason = b"authentication failed";
            stream.write_all(&(reason.len() as u32).to_be_bytes())?;
            stream.write_all(reason)?;
        }
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VNC authentication failed",
        ));
    }
    stream.write_all(&0u32.to_be_bytes())?;
    stream.set_read_timeout(None)?;
    stream.set_write_timeout(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_checks_every_supported_protocol_version() {
        assert!(password_key(b"").is_err());
        assert!(password_key(b"123456789").is_err());
        assert_eq!(
            response(&[0; 8], [0; 16]),
            [
                0x8c, 0xa6, 0x4d, 0xe9, 0xc1, 0xb1, 0x23, 0xa7, 0x8c, 0xa6, 0x4d, 0xe9, 0xc1, 0xb1,
                0x23, 0xa7
            ]
        );
        for minor in [3, 7, 8] {
            for correct in [true, false] {
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
                client
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let server = std::thread::spawn(move || {
                    let (mut stream, _) = listener.accept().unwrap();
                    handshake(&mut stream, &password_key(b"secret").unwrap()).is_ok()
                });
                let mut version = [0; 12];
                client.read_exact(&mut version).unwrap();
                client
                    .write_all(format!("RFB 003.00{minor}\n").as_bytes())
                    .unwrap();
                if minor == 3 {
                    let mut kind = [0; 4];
                    client.read_exact(&mut kind).unwrap();
                    assert_eq!(u32::from_be_bytes(kind), 2);
                } else {
                    let mut offered = [0; 2];
                    client.read_exact(&mut offered).unwrap();
                    assert_eq!(offered, [1, 2]);
                    client.write_all(&[2]).unwrap();
                }
                let mut challenge = [0; 16];
                client.read_exact(&mut challenge).unwrap();
                let key = password_key(if correct { b"secret" } else { b"wrong!" }).unwrap();
                client.write_all(&response(&key, challenge)).unwrap();
                let mut status = [0; 4];
                client.read_exact(&mut status).unwrap();
                assert_eq!(u32::from_be_bytes(status), u32::from(!correct));
                assert_eq!(server.join().unwrap(), correct);
            }
        }
    }
}
