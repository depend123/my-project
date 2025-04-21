use wasm_bindgen::prelude::*;
use js_sys::{Array, Uint8Array};
use std::collections::HashMap;
use std::io::Cursor;

// Constants for MessagePack marker bytes
const MSGPACK_FIXSTR_MASK: u8 = 0b10100000;
const MSGPACK_FIXSTR_PREFIX: u8 = 0b10100000;
const MSGPACK_STR8: u8 = 0xd9;
const MSGPACK_STR16: u8 = 0xda;
const MSGPACK_STR32: u8 = 0xdb;

const MSGPACK_POSITIVE_FIXINT_MASK: u8 = 0b10000000;
const MSGPACK_POSITIVE_FIXINT: u8 = 0b00000000;
const MSGPACK_UINT8: u8 = 0xcc;
const MSGPACK_UINT16: u8 = 0xcd;
const MSGPACK_UINT32: u8 = 0xce;
const MSGPACK_UINT64: u8 = 0xcf;

const MSGPACK_NEGATIVE_FIXINT_PREFIX: u8 = 0xe0;
const MSGPACK_NEGATIVE_FIXINT_MASK: u8 = 0xe0;
const MSGPACK_INT8: u8 = 0xd0;
const MSGPACK_INT16: u8 = 0xd1;
const MSGPACK_INT32: u8 = 0xd2;
const MSGPACK_INT64: u8 = 0xd3;

const MSGPACK_NIL: u8 = 0xc0;
const MSGPACK_FLOAT32: u8 = 0xca;
const MSGPACK_FLOAT64: u8 = 0xcb;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Helper macro for logging
macro_rules! console_log {
    ($($t:tt)*) => (log(&format!($($t)*)))
}

// Helper functions to check MessagePack marker types
fn is_str_marker(marker: u8) -> bool {
    (marker & MSGPACK_FIXSTR_MASK) == MSGPACK_FIXSTR_PREFIX || 
    marker == MSGPACK_STR8 || 
    marker == MSGPACK_STR16 || 
    marker == MSGPACK_STR32
}

fn is_uint_marker(marker: u8) -> bool {
    (marker & MSGPACK_POSITIVE_FIXINT_MASK) == MSGPACK_POSITIVE_FIXINT || 
    marker == MSGPACK_UINT8 || 
    marker == MSGPACK_UINT16 || 
    marker == MSGPACK_UINT32 || 
    marker == MSGPACK_UINT64
}

fn is_int_marker(marker: u8) -> bool {
    (marker & MSGPACK_NEGATIVE_FIXINT_MASK) == MSGPACK_NEGATIVE_FIXINT_PREFIX || 
    marker == MSGPACK_INT8 || 
    marker == MSGPACK_INT16 || 
    marker == MSGPACK_INT32 || 
    marker == MSGPACK_INT64
}

/// Encode a JavaScript object to MessagePack binary format
#[wasm_bindgen]
pub fn encode(value: &JsValue) -> Result<Uint8Array, JsValue> {
    console_log!("Encoding to MessagePack");
    
    // Convert JS value to Rust value
    let rust_value = js_to_rust_value(value)?;
    
    // Encode to MessagePack
    let mut buf = Vec::new();
    match rmp_serde::encode::write_named(&mut buf, &rust_value) {
        Ok(_) => {
            // Convert Vec<u8> to Uint8Array
            let result = Uint8Array::new_with_length(buf.len() as u32);
            result.copy_from(&buf);
            Ok(result)
        },
        Err(e) => {
            let error_msg = format!("MessagePack encoding error: {}", e);
            Err(JsValue::from_str(&error_msg))
        }
    }
}

/// Decode MessagePack binary format to a JavaScript object
#[wasm_bindgen]
pub fn decode(data: &Uint8Array) -> Result<JsValue, JsValue> {
    // Convert Uint8Array to Vec<u8>
    let len = data.length() as usize;
    let mut buf = vec![0u8; len];
    data.copy_to(&mut buf);
    
    console_log!("Decoding MessagePack data of length: {}", len);
    
    // Decode from MessagePack
    match rmp_serde::from_slice::<serde_json::Value>(&buf) {
        Ok(rust_value) => {
            // Convert Rust value to JS value
            rust_to_js_value(&rust_value)
        },
        Err(e) => {
            let error_msg = format!("MessagePack decoding error: {}", e);
            Err(JsValue::from_str(&error_msg))
        }
    }
}

// Helper function to convert JS value to Rust serde_json::Value
fn js_to_rust_value(value: &JsValue) -> Result<serde_json::Value, JsValue> {
    if value.is_null() {
        return Ok(serde_json::Value::Null);
    }
    
    if value.is_undefined() {
        return Ok(serde_json::Value::Null);
    }
    
    if let Some(val) = value.as_bool() {
        return Ok(serde_json::Value::Bool(val));
    }
    
    if let Some(val) = value.as_f64() {
        if val.fract() == 0.0 && val >= i64::MIN as f64 && val <= i64::MAX as f64 {
            return Ok(serde_json::Value::Number(serde_json::Number::from(val as i64)));
        }
        return Ok(serde_json::Value::Number(serde_json::Number::from_f64(val).unwrap()));
    }
    
    if let Some(val) = value.as_string() {
        return Ok(serde_json::Value::String(val));
    }
    
    if js_sys::Array::is_array(value) {
        let array = js_sys::Array::from(value);
        let length = array.length();
        let mut values = Vec::with_capacity(length as usize);
        
        for i in 0..length {
            let item = array.get(i);
            let rust_item = js_to_rust_value(&item)?;
            values.push(rust_item);
        }
        
        return Ok(serde_json::Value::Array(values));
    }
    
    if value.is_object() {
        let js_obj = js_sys::Object::from(value.clone());
        let keys = js_sys::Object::keys(&js_obj);
        let length = keys.length();
        let mut map = HashMap::new();
        
        for i in 0..length {
            let key = keys.get(i).as_string().unwrap();
            let js_val = js_sys::Reflect::get(&js_obj, &JsValue::from_str(&key))?;
            let rust_val = js_to_rust_value(&js_val)?;
            map.insert(key, rust_val);
        }
        
        return Ok(serde_json::Value::Object(serde_json::Map::from_iter(map)));
    }
    
    Err(JsValue::from_str("Unsupported JavaScript value type"))
}

// Helper function to convert Rust serde_json::Value to JS value
fn rust_to_js_value(value: &serde_json::Value) -> Result<JsValue, JsValue> {
    match value {
        serde_json::Value::Null => Ok(JsValue::null()),
        serde_json::Value::Bool(b) => Ok(JsValue::from_bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(JsValue::from_f64(i as f64))
            } else if let Some(f) = n.as_f64() {
                Ok(JsValue::from_f64(f))
            } else {
                Err(JsValue::from_str("Unsupported number type"))
            }
        },
        serde_json::Value::String(s) => Ok(JsValue::from_str(s)),
        serde_json::Value::Array(arr) => {
            let js_array = Array::new_with_length(arr.len() as u32);
            for (i, val) in arr.iter().enumerate() {
                let js_val = rust_to_js_value(val)?;
                js_array.set(i as u32, js_val);
            }
            Ok(js_array.into())
        },
        serde_json::Value::Object(obj) => {
            let js_obj = js_sys::Object::new();
            for (key, val) in obj {
                let js_val = rust_to_js_value(val)?;
                js_sys::Reflect::set(&js_obj, &JsValue::from_str(key), &js_val)?;
            }
            Ok(js_obj.into())
        },
    }
}

/// Specialized function for encoding an array-format message (like the server uses)
#[wasm_bindgen]
pub fn encode_array_message(message_type: &str, values: Array) -> Result<Uint8Array, JsValue> {
    let mut buf = Vec::new();
    
    // Write array length (number of values + 1 for the message type)
    let array_len = values.length() as u32 + 1;
    rmp::encode::write_array_len(&mut buf, array_len).map_err(|e| {
        JsValue::from_str(&format!("Failed to write array length: {}", e))
    })?;
    
    // Write message type
    rmp::encode::write_str(&mut buf, message_type).map_err(|e| {
        JsValue::from_str(&format!("Failed to write message type: {}", e))
    })?;
    
    // Write each value
    for i in 0..values.length() {
        let value = values.get(i);
        
        if let Some(num) = value.as_f64() {
            // For numbers, check if it's an integer
            if num.fract() == 0.0 && num >= 0.0 && num <= u32::MAX as f64 {
                rmp::encode::write_u32(&mut buf, num as u32).map_err(|e| {
                    JsValue::from_str(&format!("Failed to write u32: {}", e))
                })?;
            } else {
                rmp::encode::write_f64(&mut buf, num).map_err(|e| {
                    JsValue::from_str(&format!("Failed to write f64: {}", e))
                })?;
            }
        } else if let Some(s) = value.as_string() {
            rmp::encode::write_str(&mut buf, &s).map_err(|e| {
                JsValue::from_str(&format!("Failed to write string: {}", e))
            })?;
        } else if value.is_null() || value.is_undefined() {
            rmp::encode::write_nil(&mut buf).map_err(|e| {
                JsValue::from_str(&format!("Failed to write nil: {}", e))
            })?;
        } else {
            return Err(JsValue::from_str("Unsupported value type in array"));
        }
    }
    
    // Convert to Uint8Array
    let result = Uint8Array::new_with_length(buf.len() as u32);
    result.copy_from(&buf);
    Ok(result)
}

/// Decode an array-format message (compatible with the server's format)
#[wasm_bindgen]
pub fn decode_array_message(data: &Uint8Array) -> Result<JsValue, JsValue> {
    // Convert Uint8Array to Vec<u8>
    let len = data.length() as usize;
    let mut buf = vec![0u8; len];
    data.copy_to(&mut buf);
    
    // Create a cursor for reading
    let mut cursor = Cursor::new(&buf);
    
    // Read array length
    let array_len = rmp::decode::read_array_len(&mut cursor)
        .map_err(|e| JsValue::from_str(&format!("Failed to read array length: {}", e)))?;
    
    // Create result array
    let result = Array::new();
    
    // Try to read message type (first element)
    // Read the first element (should be a string - the message type)
    let pos = cursor.position() as usize;
    if pos >= buf.len() {
        return Err(JsValue::from_str("Unexpected end of data"));
    }
    
    // Special case for the first element (message type)
    let marker = buf[pos];
    if is_str_marker(marker) {
        // It's a string - read it manually since we can't use is_str_marker directly
        let msg_type = match marker {
            // Handle fixstr (0xa0-0xbf)
            m if (m & MSGPACK_FIXSTR_MASK) == MSGPACK_FIXSTR_PREFIX => {
                let str_len = (m & 0x1f) as usize; // Length is in the lower 5 bits
                cursor.set_position((pos + 1) as u64); // Skip the marker
                
                if pos + 1 + str_len > buf.len() {
                    return Err(JsValue::from_str("String data out of bounds"));
                }
                
                let str_bytes = &buf[pos + 1..pos + 1 + str_len];
                let s = String::from_utf8(str_bytes.to_vec())
                    .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))?;
                
                cursor.set_position((pos + 1 + str_len) as u64); // Position after the string
                s
            },
            // Handle str8 (0xd9)
            MSGPACK_STR8 => {
                if pos + 2 > buf.len() {
                    return Err(JsValue::from_str("String length out of bounds"));
                }
                
                let str_len = buf[pos + 1] as usize;
                if pos + 2 + str_len > buf.len() {
                    return Err(JsValue::from_str("String data out of bounds"));
                }
                
                let str_bytes = &buf[pos + 2..pos + 2 + str_len];
                let s = String::from_utf8(str_bytes.to_vec())
                    .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))?;
                
                cursor.set_position((pos + 2 + str_len) as u64);
                s
            },
            // Add handlers for str16 and str32 if needed
            _ => return Err(JsValue::from_str(&format!("Unsupported string marker: 0x{:02x}", marker))),
        };
        
        result.push(&JsValue::from_str(&msg_type));
    } else {
        return Err(JsValue::from_str("First element must be a string (message type)"));
    }
    
    // Read remaining elements
    for i in 1..array_len {
        let pos = cursor.position() as usize;
        if pos >= buf.len() {
            console_log!("Warning: Expected {} elements, but found only {}", array_len, i);
            break;
        }
        
        let marker = buf[pos];
        
        let value = if is_str_marker(marker) {
            // Similar implementation as above for string reading
            // For simplicity, we'll limit to fixstr and str8
            let s = match marker {
                m if (m & MSGPACK_FIXSTR_MASK) == MSGPACK_FIXSTR_PREFIX => {
                    let str_len = (m & 0x1f) as usize;
                    cursor.set_position((pos + 1) as u64);
                    
                    if pos + 1 + str_len > buf.len() {
                        return Err(JsValue::from_str("String data out of bounds"));
                    }
                    
                    let str_bytes = &buf[pos + 1..pos + 1 + str_len];
                    let s = String::from_utf8(str_bytes.to_vec())
                        .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))?;
                    
                    cursor.set_position((pos + 1 + str_len) as u64);
                    s
                },
                MSGPACK_STR8 => {
                    if pos + 2 > buf.len() {
                        return Err(JsValue::from_str("String length out of bounds"));
                    }
                    
                    let str_len = buf[pos + 1] as usize;
                    if pos + 2 + str_len > buf.len() {
                        return Err(JsValue::from_str("String data out of bounds"));
                    }
                    
                    let str_bytes = &buf[pos + 2..pos + 2 + str_len];
                    let s = String::from_utf8(str_bytes.to_vec())
                        .map_err(|e| JsValue::from_str(&format!("Invalid UTF-8: {}", e)))?;
                    
                    cursor.set_position((pos + 2 + str_len) as u64);
                    s
                },
                _ => return Err(JsValue::from_str(&format!("Unsupported string marker: 0x{:02x}", marker))),
            };
            JsValue::from_str(&s)
        } else if is_uint_marker(marker) {
            // Handle various integer formats
            let n = match marker {
                // Positive fixint (0x00-0x7f)
                m if (m & MSGPACK_POSITIVE_FIXINT_MASK) == MSGPACK_POSITIVE_FIXINT => {
                    cursor.set_position((pos + 1) as u64);
                    m as u32
                },
                // uint8 (0xcc)
                MSGPACK_UINT8 => {
                    if pos + 2 > buf.len() {
                        return Err(JsValue::from_str("Uint8 out of bounds"));
                    }
                    let n = buf[pos + 1] as u32;
                    cursor.set_position((pos + 2) as u64);
                    n
                },
                // uint16 (0xcd)
                MSGPACK_UINT16 => {
                    if pos + 3 > buf.len() {
                        return Err(JsValue::from_str("Uint16 out of bounds"));
                    }
                    let n = ((buf[pos + 1] as u32) << 8) | (buf[pos + 2] as u32);
                    cursor.set_position((pos + 3) as u64);
                    n
                },
                // uint32 (0xce)
                MSGPACK_UINT32 => {
                    if pos + 5 > buf.len() {
                        return Err(JsValue::from_str("Uint32 out of bounds"));
                    }
                    let n = ((buf[pos + 1] as u32) << 24) | 
                            ((buf[pos + 2] as u32) << 16) | 
                            ((buf[pos + 3] as u32) << 8) | 
                            (buf[pos + 4] as u32);
                    cursor.set_position((pos + 5) as u64);
                    n
                },
                _ => return Err(JsValue::from_str(&format!("Unsupported uint marker: 0x{:02x}", marker))),
            };
            JsValue::from_f64(n as f64)
        } else if is_int_marker(marker) {
            // Similar implementation for signed integers
            let n = match marker {
                // Negative fixint (0xe0-0xff)
                m if (m & MSGPACK_NEGATIVE_FIXINT_MASK) == MSGPACK_NEGATIVE_FIXINT_PREFIX => {
                    cursor.set_position((pos + 1) as u64);
                    (m as i8) as i32 // Convert through i8 to ensure proper sign extension
                },
                // int8 (0xd0)
                MSGPACK_INT8 => {
                    if pos + 2 > buf.len() {
                        return Err(JsValue::from_str("Int8 out of bounds"));
                    }
                    let n = (buf[pos + 1] as i8) as i32;
                    cursor.set_position((pos + 2) as u64);
                    n
                },
                // int16 (0xd1)
                MSGPACK_INT16 => {
                    if pos + 3 > buf.len() {
                        return Err(JsValue::from_str("Int16 out of bounds"));
                    }
                    let n = ((buf[pos + 1] as i16) << 8 | (buf[pos + 2] as i16)) as i32;
                    cursor.set_position((pos + 3) as u64);
                    n
                },
                // int32 (0xd2)
                MSGPACK_INT32 => {
                    if pos + 5 > buf.len() {
                        return Err(JsValue::from_str("Int32 out of bounds"));
                    }
                    let n = ((buf[pos + 1] as i32) << 24) | 
                            ((buf[pos + 2] as i32) << 16) | 
                            ((buf[pos + 3] as i32) << 8) | 
                            (buf[pos + 4] as i32);
                    cursor.set_position((pos + 5) as u64);
                    n
                },
                _ => return Err(JsValue::from_str(&format!("Unsupported int marker: 0x{:02x}", marker))),
            };
            JsValue::from_f64(n as f64)
        } else if marker == MSGPACK_FLOAT64 {
            // Float64 (0xcb)
            if pos + 9 > buf.len() {
                return Err(JsValue::from_str("Float64 out of bounds"));
            }
            
            // Read bytes as big-endian f64
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&buf[pos + 1..pos + 9]);
            let n = f64::from_be_bytes(bytes);
            
            cursor.set_position((pos + 9) as u64);
            JsValue::from_f64(n)
        } else if marker == MSGPACK_NIL {
            // nil (0xc0)
            cursor.set_position((pos + 1) as u64);
            JsValue::null()
        } else {
            return Err(JsValue::from_str(&format!("Unsupported marker: 0x{:02x}", marker)));
        };
        
        result.push(&value);
    }
    
    Ok(result.into())
}