use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use serde::{Serialize, Deserialize};

// Явно импортируем rmp и rmp_serde
extern crate rmp_serde;
extern crate rmp;

// Определяем тип ошибки, который можно безопасно передавать между потоками
type BoxError = Box<dyn std::error::Error + Send + Sync>;

// Game state and client management
type ClientId = u32;
type Clients = Arc<Mutex<HashMap<ClientId, mpsc::UnboundedSender<Message>>>>;

// Message structures for MessagePack
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
enum GameMessage {
    #[serde(rename = "Init")]
    Init { id: ClientId },
    
    #[serde(rename = "Joined")]
    Joined { id: ClientId },
    
    #[serde(rename = "Left")]
    Left { id: ClientId },
    
    #[serde(rename = "PlayerMove")]
    PlayerMove { 
        id: ClientId, 
        x: f64, 
        y: f64, 
        #[serde(rename = "vel_x")] 
        vel_x: f64, 
        #[serde(rename = "vel_y")] 
        vel_y: f64 
    },
    
    #[serde(rename = "Kick")]
    Kick { 
        id: ClientId, 
        x: f64, 
        y: f64, 
        #[serde(rename = "dirX")] 
        dir_x: f64, 
        #[serde(rename = "dirY")] 
        dir_y: f64 
    },
}

// Client message structures
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "Move")]
    Move { 
        x: f64, 
        y: f64, 
        #[serde(rename = "vel_x")] 
        vel_x: f64, 
        #[serde(rename = "vel_y")] 
        vel_y: f64 
    },
    
    #[serde(rename = "Kick")]
    Kick { 
        x: f64, 
        y: f64, 
        #[serde(rename = "dirX")] 
        dir_x: f64, 
        #[serde(rename = "dirY")] 
        dir_y: f64 
    },
}

// Функции для создания сообщений в формате массива

// Для Init и Joined (только ID)
fn create_simple_message(msg_type: &str, id: ClientId) -> Result<Vec<u8>, BoxError> {
    let mut buf = Vec::new();
    
    // Массив из 2 элементов: тип и ID
    rmp::encode::write_array_len(&mut buf, 2)?;
    rmp::encode::write_str(&mut buf, msg_type)?;
    rmp::encode::write_u32(&mut buf, id)?;
    
    Ok(buf)
}

// Для PlayerMove (id, x, y, vel_x, vel_y)
fn create_move_message(id: ClientId, x: f64, y: f64, vel_x: f64, vel_y: f64) -> Result<Vec<u8>, BoxError> {
    let mut buf = Vec::new();
    
    // Массив из 6 элементов: тип, id, x, y, vel_x, vel_y
    rmp::encode::write_array_len(&mut buf, 6)?;
    rmp::encode::write_str(&mut buf, "PlayerMove")?;
    rmp::encode::write_u32(&mut buf, id)?;
    rmp::encode::write_f64(&mut buf, x)?;
    rmp::encode::write_f64(&mut buf, y)?;
    rmp::encode::write_f64(&mut buf, vel_x)?;
    rmp::encode::write_f64(&mut buf, vel_y)?;
    
    Ok(buf)
}

// Для Kick (id, x, y, dirX, dirY)
fn create_kick_message(id: ClientId, x: f64, y: f64, dir_x: f64, dir_y: f64) -> Result<Vec<u8>, BoxError> {
    let mut buf = Vec::new();
    
    // Массив из 6 элементов: тип, id, x, y, dirX, dirY
    rmp::encode::write_array_len(&mut buf, 6)?;
    rmp::encode::write_str(&mut buf, "Kick")?;
    rmp::encode::write_u32(&mut buf, id)?;
    rmp::encode::write_f64(&mut buf, x)?;
    rmp::encode::write_f64(&mut buf, y)?;
    rmp::encode::write_f64(&mut buf, dir_x)?;
    rmp::encode::write_f64(&mut buf, dir_y)?;
    
    Ok(buf)
}

// Вспомогательная функция для HEX-дампа бинарных данных
fn hex_dump(data: &[u8], max_bytes: usize) -> String {
    let bytes_to_show = std::cmp::min(data.len(), max_bytes);
    let mut result = String::new();
    
    for i in 0..bytes_to_show {
        result.push_str(&format!("{:02x} ", data[i]));
        if (i + 1) % 16 == 0 && i + 1 < bytes_to_show {
            result.push('\n');
        }
    }
    
    if data.len() > max_bytes {
        result.push_str("...");
    }
    
    result
}

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    // Initialize WebSocket server
    let addr = "0.0.0.0:8080";
    let listener = TcpListener::bind(&addr).await?;
    println!("WebSocket server listening on: {}", addr);

    // Shared game state
    let clients: Clients = Arc::new(Mutex::new(HashMap::new()));
    let mut client_id_counter: ClientId = 0;

    // Accept WebSocket connections
    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().unwrap();
        // Assign client ID and increment counter
        let client_id = client_id_counter;
        client_id_counter += 1;
        
        // Клонируем clients для передачи в задачу
        let clients_clone = Arc::clone(&clients);
        
        // Запускаем обработку соединения в отдельной задаче
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, peer, clients_clone, client_id).await {
                eprintln!("Error in connection handler: {}", e);
            }
        });
    }

    Ok(())
}

// Обработчик соединения теперь возвращает Result<(), BoxError>
async fn handle_connection(stream: TcpStream, peer: SocketAddr, clients: Clients, client_id: ClientId) -> Result<(), BoxError> {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            eprintln!("Error during WebSocket handshake: {}", e);
            return Err(Box::new(e));
        }
    };

    println!("New WebSocket connection: {} (ID: {})", peer, client_id);

    // Split the WebSocket stream
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Create a channel for sending messages to this client
    let (client_sender, mut client_receiver) = mpsc::unbounded_channel();
    
    // Store the new client's sender
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.insert(client_id, client_sender);
    }

    // Send initialization message to the new client using MessagePack as array
    let packed_msg = create_simple_message("Init", client_id)?;
    println!("Init message for client {}, raw bytes: {}", client_id, hex_dump(&packed_msg, 32));
    
    if let Err(e) = ws_sender.send(Message::Binary(packed_msg)).await {
        eprintln!("Error sending init message: {}", e);
        return Err(Box::new(e));
    }
    println!("Sent init message to client {}", client_id);
    
    // Notify all clients about the new player - using array format
    let packed_msg = create_simple_message("Joined", client_id)?;
    println!("Join message for client {}, raw bytes: {}", client_id, hex_dump(&packed_msg, 32));
    
    broadcast_message(&clients, client_id, Message::Binary(packed_msg)).await;
    println!("Broadcast join message for client {}", client_id);
    
    // Spawn a task to forward messages from the client_receiver to the WebSocket
    let client_id_clone = client_id;
    let forward_task = tokio::spawn(async move {
        while let Some(msg) = client_receiver.recv().await {
            if let Err(e) = ws_sender.send(msg).await {
                eprintln!("Error sending message to client {}: {}", client_id_clone, e);
                break;
            }
        }
    });

    // Process incoming WebSocket messages
    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(msg) => {
                if let Message::Binary(data) = msg {
                    // Добавляем отладочную информацию
                    println!("Received binary message from client {}, size: {} bytes", client_id, data.len());
                    println!("Message raw bytes: {}", hex_dump(&data, 32));
                    
                    match rmp_serde::from_slice::<ClientMessage>(&data) {
                        Ok(client_msg) => {
                            match client_msg {
                                ClientMessage::Move { x, y, vel_x, vel_y } => {
                                    println!("Client {} sent Move: x={}, y={}, vel_x={}, vel_y={}", 
                                             client_id, x, y, vel_x, vel_y);
                                    
                                    // Отправляем PlayerMove в формате массива
                                    if let Ok(packed_msg) = create_move_message(client_id, x, y, vel_x, vel_y) {
                                        println!("PlayerMove message, raw bytes: {}", hex_dump(&packed_msg, 32));
                                        broadcast_message(&clients, client_id, Message::Binary(packed_msg)).await;
                                    }
                                },
                                ClientMessage::Kick { x, y, dir_x, dir_y } => {
                                    println!("Client {} sent Kick: x={}, y={}, dirX={}, dirY={}", 
                                             client_id, x, y, dir_x, dir_y);
                                    
                                    // Отправляем Kick в формате массива
                                    if let Ok(packed_msg) = create_kick_message(client_id, x, y, dir_x, dir_y) {
                                        println!("Kick message, raw bytes: {}", hex_dump(&packed_msg, 32));
                                        broadcast_message(&clients, client_id, Message::Binary(packed_msg)).await;
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("Failed to deserialize MessagePack data: {}", e);
                            // Отладка: печатаем первые байты сообщения для анализа
                            if !data.is_empty() {
                                eprintln!("Raw message bytes: {}", hex_dump(&data, data.len()));
                            }
                            
                            // Пробуем ручную десериализацию для отладки
                            if let Ok(raw_value) = rmp_serde::from_slice::<serde_json::Value>(&data) {
                                eprintln!("Raw deserialized as JSON: {:?}", raw_value);
                            }
                        }
                    }
                }
            },
            Err(e) => {
                eprintln!("WebSocket error for client {}: {}", client_id, e);
                break;
            }
        }
    }

    // Client disconnected
    {
        let mut clients_lock = clients.lock().unwrap();
        clients_lock.remove(&client_id);
    }

    // Notify other clients about the disconnection - using array format
    if let Ok(packed_msg) = create_simple_message("Left", client_id) {
        broadcast_message(&clients, client_id, Message::Binary(packed_msg)).await;
    }

    // Cancel the send task
    forward_task.abort();
    
    println!("WebSocket connection closed: {} (ID: {})", peer, client_id);
    
    Ok(())
}

async fn broadcast_message(clients: &Clients, exclude_id: ClientId, message: Message) {
    let clients_lock = clients.lock().unwrap();
    let client_count = clients_lock.len();
    
    let mut sent_count = 0;
    for (id, sender) in clients_lock.iter() {
        if *id != exclude_id {
            if sender.send(message.clone()).is_ok() {
                sent_count += 1;
            }
        }
    }
    
    println!("Broadcast message sent to {}/{} clients (excluding {})", 
             sent_count, client_count - 1, exclude_id);
}