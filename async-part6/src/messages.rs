#[cfg(feature = "compression")]
use {
    flate2::{read::GxDecoder, write::GxEncoder, Compression},
    std::io::{Read, Write},
};

use serde::{Deserialize, Serialize};

// Custom error type to handle different error types
pub enum DeserializeError {
    InsufficientData,
    #[cfg(feature = "compression")]
    DecompressionFailed,
    DeserializationFailed
}

#[allow(dead_code)]
pub trait SensorMessage<T> {
    fn get_value(&self) -> Option<T>;
    fn is_error(&self) -> bool;
    fn is_unusable(&self) -> bool;
    fn get_sensor_name(&self) -> Option<&String>;
    fn set_value(&mut self, val: T) -> &mut Self;
    fn set_error(&mut self) -> &mut Self;
    fn set_unusable(&mut self, fallback_value: T) -> &mut Self;
    fn set_sensor_name(&mut self, content: String) -> &mut Self;

    fn to_values(messages: Vec<Message<T>>) -> Vec<T>
    where
        T: Copy,
        {
            messages.into_iter().filter_map(|msg| msg.get_value()).collect()
        }
    
    fn to_vec(&self) -> Vec<u8>
    where
        Self: Serialize,
    {
        let serialized_data = bincode::serialize(self).expect("Failed to serialize message");
        #[cfg(feature = "compression")]
        let final_data = {
            let mut encoder = GxEncoder::new(Vec::new(), Compression::fast());
            encoder
                .write_all(&serialized_data)
                .expect("Failed to compress data");
            encoder.finish().expect("Failed to finish compression")
        };

        #[cfg(not(feature = "compression"))]
        let final_data = serialized_data;

        let length = final_data.len() as u32;
        let mut length_bytes = length.to_be_bytes().to_vec();

        length_bytes.extend(final_data);

        length_bytes
    }

    fn from_vec(buffer: &[u8]) -> Result<Self, DeserializeError>
    where
        Self: Sized + for<'a> Deserialize<'a>,
    {
        if buffer.len() < 4 {
            return Err(DeserializeError::InsufficientData);
        }

        let length = u32::from_be_bytes(
            buffer[..4]
                .try_into()
                .map_err(|_| DeserializeError::InsufficientData)?
        ) as usize;

        if buffer.len() < length + 4 {
            return Err(DeserializeError::InsufficientData);
        }

        let message_data = &buffer[4..4+length];

        #[cfg(feature = "compression")]
        {
            let mut decoder = GxDecoder::new(message_data);
            let mut decompressed_data = Vec::new();
            decoder
                .read_to_end(&mut decompressed_data)
                .map_err(|_| DeserializeError::DecompressionFailed)?;

            bincode::deserialize(&mut decompressed_data)
                .map_err(|_| DeserializeError::DeserializationFailed)
        }

        #[cfg(not(feature = "compression"))]
        {
            bincode::deserialize(message_data)
                .map_err(|_| DeserializeError::DeserializationFailed)

        }
    }
}

#[derive(Serialize, Deserialize)]
enum InnerMessage<T> {
    Value(T),
    Error,
    NotValid(T),
}

#[derive(Serialize, Deserialize)]
pub struct Message<T> {
    message: InnerMessage<T>,
    sensor_name: String,
}

impl<T> Message<T> {
    pub fn new(value: T, sensor_name: String) -> Message<T> {
        Message { 
            message: InnerMessage::Value(value),
            sensor_name, 
        }
    }
}

impl<T> SensorMessage<T> for Message<T>
where
    T: Copy,
{
    fn get_value(&self) -> Option<T> {
        match self.message {
            InnerMessage::Value(val) | InnerMessage::NotValid(val) => Some(val),
            _ => None,
        }
    }

    fn is_error(&self) -> bool {
        matches!(self.message, InnerMessage::Error)
    }

    fn is_unusable(&self) -> bool {
        matches!(self.message, InnerMessage::NotValid(_))
    }

    fn get_sensor_name(&self) -> Option<&String> {  
        Some(&self.sensor_name)
    }

    fn set_value(&mut self, val: T) -> &mut Self {
        self.message = InnerMessage::Value(val);
        self
    }

    fn set_error(&mut self) -> &mut Self {  
        self.message = InnerMessage::Error;
        self
    }

    fn set_unusable(&mut self, fallback_value: T) -> &mut Self {
        if let InnerMessage::Value(val) = self.message {
            self.message = InnerMessage::NotValid(val);
        } else {
            self.message = InnerMessage::NotValid(fallback_value);
        }
        self
    }

    fn set_sensor_name(&mut self, content: String) -> &mut Self {
        self.sensor_name = content;
        self
    }
}