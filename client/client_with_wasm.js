// Game constants
const BALL_RADIUS = 15;
const AVATAR_RADIUS = 20;
const BALL_FRICTION = 0.96;
const KICK_POWER = 4.0;
const PLAYER_SPEED = 120; // pixels per second

// Fixed timestep for game logic (30 FPS, like Flash)
const FIXED_TIMESTEP = 1000 / 30; // ~33.33ms
const SMOOTHING_FACTOR = 0.2; // Степень сглаживания для интерполяции

// Global variables for WASM module
let wasmModule = null;
let wasmReady = false;

// Game state
let socket = null;
let playerId = null;
let players = {}; // Теперь содержит logical и visual позиции
let ball = {
    logical: { x: 400, y: 300, vx: 0, vy: 0 },
    visual: { x: 400, y: 300 }
};
let lastClickTarget = { x: null, y: null };
let clickWasOnBall = false;
let lastKickTime = 0;

// Timer and accumulator for fixed timestep
let lastFixedUpdateTime = 0;
let accumulator = 0;
let fixedUpdateInterval = null;

// Canvas setup
const canvas = document.getElementById("game-canvas");
const ctx = canvas.getContext("2d");

// Отладка
const DEBUG = true;
function debugLog(...args) {
    if (DEBUG) console.log('[DEBUG]', ...args);
}

// Generate placeholder images
function createPlaceholderImage(color, size) {
    const img = new Image(size, size);
    const tempCanvas = document.createElement("canvas");
    tempCanvas.width = size;
    tempCanvas.height = size;
    const tempCtx = tempCanvas.getContext("2d");
    tempCtx.fillStyle = color;
    tempCtx.beginPath();
    tempCtx.arc(size/2, size/2, size/2, 0, Math.PI * 2);
    tempCtx.fill();
    img.src = tempCanvas.toDataURL();
    return img;
}

// Create placeholder images
const fieldImg = new Image(800, 600);
const tempCanvas = document.createElement("canvas");
tempCanvas.width = 800;
tempCanvas.height = 600;
const tempCtx = tempCanvas.getContext("2d");
tempCtx.fillStyle = "#7CFC00"; // Light green field
tempCtx.fillRect(0, 0, 800, 600);
// Add field lines
tempCtx.strokeStyle = "white";
tempCtx.lineWidth = 2;
tempCtx.strokeRect(50, 50, 700, 500); // Field boundary
tempCtx.beginPath();
tempCtx.arc(400, 300, 100, 0, Math.PI * 2); // Center circle
tempCtx.stroke();
tempCtx.beginPath();
tempCtx.moveTo(400, 50);
tempCtx.lineTo(400, 550); // Middle line
tempCtx.stroke();
fieldImg.src = tempCanvas.toDataURL();

const ballImg = createPlaceholderImage("#FFFFFF", BALL_RADIUS * 2); // White ball
const avatarImg = createPlaceholderImage("#3498DB", AVATAR_RADIUS * 2); // Blue avatar

// Initialize game
async function init() {
    // Load WASM module first
    try {
        // Dynamic import of the WebAssembly module
        const wasm = await import('./wasm/msgpack_wasm.js');
        // Initialize the module
        wasmModule = await wasm.default();
        wasmReady = true;
        console.log("WebAssembly MessagePack module loaded successfully");
    } catch (error) {
        console.error("Failed to load WebAssembly MessagePack module:", error);
        // Fallback to msgpack-lite if WebAssembly failed to load
        console.log("Falling back to msgpack-lite");
    }
    
    // Connect to WebSocket server once WASM is ready
    connectToServer();
}

// Connect to WebSocket server
function connectToServer() {
    // Connect to WebSocket server
    socket = new WebSocket("ws://46.8.52.91:8080");
    socket.binaryType = "arraybuffer";
    
    // WebSocket event handlers
    socket.onopen = () => {
        console.log("Connected to server");
    };
    
    socket.onclose = () => {
        console.log("Disconnected from server");
    };
    
    socket.onerror = (error) => {
        console.error("WebSocket error:", error);
    };
    
    socket.onmessage = (event) => {
        try {
            // Use WebAssembly for decoding if available
            const rawData = new Uint8Array(event.data);
            debugLog("Raw message bytes:", Array.from(rawData.slice(0, 20)));
            
            let parsedData;
            if (wasmReady) {
                // Use our WebAssembly decoder
                parsedData = wasmModule.decode_array_message(rawData);
                // Convert array format to object format for compatibility
                if (Array.isArray(parsedData)) {
                    parsedData = convertArrayToObject(parsedData);
                }
            } else {
                // Fallback to msgpack-lite
                parsedData = msgpack.decode(rawData);
                // If data is an array, convert it to object
                if (Array.isArray(parsedData)) {
                    parsedData = convertArrayToObject(parsedData);
                }
            }
            
            debugLog("Decoded message:", parsedData);
            handleMessage(parsedData);
        } catch (e) {
            console.error("Error processing message:", e);
            if (event.data instanceof ArrayBuffer) {
                const bytes = new Uint8Array(event.data);
                console.error("Raw bytes:", Array.from(bytes));
            }
        }
    };
    
    // Set up game input
    canvas.addEventListener("mousedown", handleMouseDown);
    
    // Start render loop (high frequency, variable timestep)
    requestAnimationFrame(renderLoop);
    
    // Start fixed update loop (exactly 30 FPS for game logic)
    lastFixedUpdateTime = performance.now();
    fixedUpdateInterval = setInterval(fixedUpdate, FIXED_TIMESTEP);
}

// Helper function to convert array format messages to object format
function convertArrayToObject(data) {
    if (!Array.isArray(data) || data.length < 1) {
        return data;
    }
    
    const messageType = data[0];
    
    if (messageType === "Init" && data.length > 1) {
        return {
            type: messageType,
            id: data[1]
        };
    } else if (messageType === "Joined" && data.length > 1) {
        return {
            type: messageType,
            id: data[1]
        };
    } else if (messageType === "PlayerMove" && data.length > 5) {
        return {
            type: messageType,
            id: data[1],
            x: data[2],
            y: data[3],
            vel_x: data[4],
            vel_y: data[5]
        };
    } else if (messageType === "Kick" && data.length > 5) {
        return {
            type: messageType,
            id: data[1],
            x: data[2],
            y: data[3],
            dirX: data[4],
            dirY: data[5]
        };
    } else if (messageType === "Left" && data.length > 1) {
        return {
            type: messageType,
            id: data[1]
        };
    }
    
    return data;
}

// Fixed timestep update function (runs at exactly 30 FPS)
function fixedUpdate() {
    const now = performance.now();
    const fixedDelta = FIXED_TIMESTEP / 1000; // Convert to seconds
    
    // Update player positions with fixed timestep
    updatePlayers(fixedDelta);
    
    // Update ball physics with fixed timestep
    updateBall(fixedDelta);
    
    lastFixedUpdateTime = now;
}

// Rendering loop (runs as fast as possible)
function renderLoop(timestamp) {
    // Interpolate visual positions toward logical positions
    interpolatePositions();
    
    // Draw the game state
    draw();
    
    // Continue render loop
    requestAnimationFrame(renderLoop);
}

// Interpolate visual positions toward logical positions for smooth rendering
function interpolatePositions() {
    // Interpolate player positions
    for (let id in players) {
        const p = players[id];
        if (p.logical && p.visual) {
            // Use different smoothing factor based on player's movement state
            // When player is stopped, use a higher smoothing factor for faster convergence
            const factor = (p.logical.vel_x === 0 && p.logical.vel_y === 0) ? 0.5 : SMOOTHING_FACTOR;
            p.visual.x += (p.logical.x - p.visual.x) * factor;
            p.visual.y += (p.logical.y - p.visual.y) * factor;
        }
    }
    
    // Interpolate ball position
    ball.visual.x += (ball.logical.x - ball.visual.x) * SMOOTHING_FACTOR;
    ball.visual.y += (ball.logical.y - ball.visual.y) * SMOOTHING_FACTOR;
}

// Handle incoming messages
function handleMessage(data) {
    debugLog("Processing message:", data);
    
    // Проверка ID на валидность для всех типов сообщений кроме "Init"
    if (data.type !== "Init" && data.id !== undefined) {
        // Убедимся, что ID у нас в виде числа
        if (typeof data.id !== 'number') {
            data.id = Number(data.id);
            if (isNaN(data.id)) {
                console.error("Invalid ID in message:", data);
                return;
            }
        }
    }

    switch (data.type) {
        case "Init":
            // ИСПРАВЛЕНО: Убедимся, что ID извлекается правильно
            playerId = data.id;
            
            // Отладочная информация
            debugLog("Setting Player ID to:", playerId);
            
            // Проверка на валидность ID
            if (playerId === undefined || playerId === null) {
                console.error("Received Init message with invalid ID", data);
                return;
            }
            
            // Initialize both logical and visual positions
            players[playerId] = {
                logical: { x: 100, y: 100, vel_x: 0, vel_y: 0 },
                visual: { x: 100, y: 100 }
            };
            ball.logical = { x: 400, y: 300, vx: 0, vy: 0 };
            ball.visual = { x: 400, y: 300 };
            console.log(`Initialized as player ${playerId}`, players[playerId]);
            break;
            
        case "Joined":
            // Add new player
            if (data.id !== undefined && !players[data.id]) {
                // New players start with zero velocity
                players[data.id] = {
                    logical: { x: 100, y: 100, vel_x: 0, vel_y: 0 },
                    visual: { x: 100, y: 100 }
                };
                console.log(`Player ${data.id} joined`, players[data.id]);
            }
            break;
            
        case "Left":
            // Remove disconnected player
            if (data.id !== undefined && players[data.id]) {
                console.log(`Player ${data.id} left`);
                delete players[data.id];
            }
            break;
            
        case "PlayerMove":
            // Update other player's movement vector
            if (data.id !== undefined && data.id !== playerId) {
                if (!players[data.id]) {
                    // Create player if doesn't exist yet
                    players[data.id] = { 
                        logical: { 
                            x: data.x || 100, 
                            y: data.y || 100, 
                            vel_x: data.vel_x || 0, 
                            vel_y: data.vel_y || 0 
                        },
                        visual: { x: data.x || 100, y: data.y || 100 }
                    };
                    debugLog(`Created player ${data.id} from PlayerMove`, players[data.id]);
                } else {
                    // Update player logical position and velocity
                    if (typeof data.x === 'number') players[data.id].logical.x = data.x;
                    if (typeof data.y === 'number') players[data.id].logical.y = data.y;
                    if (typeof data.vel_x === 'number') players[data.id].logical.vel_x = data.vel_x;
                    if (typeof data.vel_y === 'number') players[data.id].logical.vel_y = data.vel_y;
                    debugLog(`Updated player ${data.id} from PlayerMove`, players[data.id]);
                }
            }
            break;
            
        case "Kick":
            // ВАЖНО: Лучшая обработка удара по мячу от другого игрока
            debugLog("Received Kick message:", data);
            
            // Применяем новые координаты мяча
            if (typeof data.x === "number") ball.logical.x = data.x;
            if (typeof data.y === "number") ball.logical.y = data.y;
            
            // Получаем направление удара (поддержка обоих форматов)
            let dirX = 0, dirY = 0;
            
            // Поддержка обоих вариантов имен свойств
            if (typeof data.dirX === 'number') dirX = data.dirX;
            else if (typeof data.dir_x === 'number') dirX = data.dir_x;
            
            if (typeof data.dirY === 'number') dirY = data.dirY;
            else if (typeof data.dir_y === 'number') dirY = data.dir_y;
            
            debugLog("Kick direction:", dirX, dirY);
            
            // Применяем скорость к мячу
            const norm = Math.hypot(dirX, dirY);
            if (norm > 0) {
                const normalizedDirX = dirX / norm;
                const normalizedDirY = dirY / norm;
                ball.logical.vx = normalizedDirX * KICK_POWER;
                ball.logical.vy = normalizedDirY * KICK_POWER;
                
                debugLog("Applied velocity to ball:", ball.logical.vx, ball.logical.vy);
            }
            break;
            
        default:
            console.warn("Unknown message type:", data.type);
            break;
    }
}

// Handle mouse clicks
function handleMouseDown(e) {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    // ИСПРАВЛЕНО: Дополнительная проверка валидности ID
    if (playerId !== null && playerId !== undefined && players[playerId]) {
        debugLog("Mouse down at", x, y, "Player ID:", playerId);
        
        const p = players[playerId];
        const distToBall = Math.hypot(ball.logical.x - x, ball.logical.y - y);
        
        // Check if the click was directly on the ball
        clickWasOnBall = distToBall < BALL_RADIUS;
        
        // Calculate movement vector (direction to clicked position)
        const dx = x - p.logical.x;
        const dy = y - p.logical.y;
        const dist = Math.hypot(dx, dy);
        
        if (dist > 0) {
            // Set velocity based on normalized direction vector
            p.logical.vel_x = (dx / dist) * PLAYER_SPEED;
            p.logical.vel_y = (dy / dist) * PLAYER_SPEED;
            debugLog("Setting velocity:", p.logical.vel_x, p.logical.vel_y);
        } else {
            // If player clicked on themselves, stop movement
            p.logical.vel_x = 0;
            p.logical.vel_y = 0;
        }
        
        // Store click target for kick logic
        lastClickTarget = { x, y };
        
        // Send move message to server with current position and velocity vector
        if (socket && socket.readyState === WebSocket.OPEN) {
            const moveMsg = { 
                type: "Move", 
                x: p.logical.x, 
                y: p.logical.y,
                vel_x: p.logical.vel_x, 
                vel_y: p.logical.vel_y 
            };
            debugLog("Sending Move:", moveMsg);
            
            let encodedMsg;
            if (wasmReady) {
                // Use WebAssembly for encoding
                const moveValues = [p.logical.x, p.logical.y, p.logical.vel_x, p.logical.vel_y];
                encodedMsg = wasmModule.encode_array_message("Move", new Array(...moveValues));
            } else {
                // Fallback to msgpack-lite
                encodedMsg = msgpack.encode(moveMsg);
            }
            
            socket.send(encodedMsg);
        }
    } else {
        console.warn("Mouse click ignored - invalid player ID or not initialized:", playerId);
    }
}

// Update player positions (called at fixed 30 FPS rate)
function updatePlayers(dt) {
    for (let id in players) {
        const p = players[id];
        
        // Skip players without logical position
        if (!p.logical) continue;
        
        // Check if local player reached target position
        if (id == playerId && lastClickTarget.x !== null) {
            const distToTarget = Math.hypot(p.logical.x - lastClickTarget.x, p.logical.y - lastClickTarget.y);
            
            // If player is close enough to target point, stop movement
            if (distToTarget < 2) {
                p.logical.vel_x = 0;
                p.logical.vel_y = 0;
                
                // Send update to server about stopping
                if (socket && socket.readyState === WebSocket.OPEN) {
                    const moveMsg = { 
                        type: "Move", 
                        x: p.logical.x, 
                        y: p.logical.y,
                        vel_x: 0, 
                        vel_y: 0 
                    };
                    
                    let encodedMsg;
                    if (wasmReady) {
                        // Use WebAssembly for encoding
                        const moveValues = [p.logical.x, p.logical.y, 0, 0];
                        encodedMsg = wasmModule.encode_array_message("Move", new Array(...moveValues));
                    } else {
                        // Fallback to msgpack-lite
                        encodedMsg = msgpack.encode(moveMsg);
                    }
                    
                    socket.send(encodedMsg);
                }
                
                // Clear click target since we've reached it
                lastClickTarget = { x: null, y: null };
            }
        }
        
        // Apply velocity to logical position
        p.logical.x += p.logical.vel_x * dt;
        p.logical.y += p.logical.vel_y * dt;
        
        // Constrain player within canvas boundaries
        const minX = AVATAR_RADIUS;
        const minY = AVATAR_RADIUS;
        const maxX = canvas.width - AVATAR_RADIUS;
        const maxY = canvas.height - AVATAR_RADIUS;
        
        // Boundary collision handling
        if (p.logical.x < minX) {
            p.logical.x = minX;
            p.logical.vel_x = 0; // Stop horizontal movement at boundary
        } else if (p.logical.x > maxX) {
            p.logical.x = maxX;
            p.logical.vel_x = 0; // Stop horizontal movement at boundary
        }
        
        if (p.logical.y < minY) {
            p.logical.y = minY;
            p.logical.vel_y = 0; // Stop vertical movement at boundary
        } else if (p.logical.y > maxY) {
            p.logical.y = maxY;
            p.logical.vel_y = 0; // Stop vertical movement at boundary
        }
        
        // Check for ball kick (only for local player)
        if (
            id == playerId &&
            lastClickTarget.x !== null &&
            !clickWasOnBall &&
            Date.now() - lastKickTime >= 500
        ) {
            const ballDist = Math.hypot(ball.logical.x - p.logical.x, ball.logical.y - p.logical.y);
            
            if (ballDist < BALL_RADIUS + AVATAR_RADIUS) {
                // Calculate kick direction
                const dx = lastClickTarget.x - p.logical.x;
                const dy = lastClickTarget.y - p.logical.y;
                const norm = Math.hypot(dx, dy);
                
                if (norm > 0) {
                    const dirX = dx / norm;
                    const dirY = dy / norm;
                    
                    // Apply kick locally
                    ball.logical.vx = dirX * KICK_POWER;
                    ball.logical.vy = dirY * KICK_POWER;
                    
                    // Send kick message to server
                    if (socket && socket.readyState === WebSocket.OPEN) {
                        const kickMsg = {
                            type: "Kick",
                            x: ball.logical.x,
                            y: ball.logical.y,
                            dirX: dirX,
                            dirY: dirY
                        };
                        debugLog("Sending Kick:", kickMsg);
                        
                        let encodedMsg;
                        if (wasmReady) {
                            // Use WebAssembly for encoding
                            const kickValues = [ball.logical.x, ball.logical.y, dirX, dirY];
                            encodedMsg = wasmModule.encode_array_message("Kick", new Array(...kickValues));
                        } else {
                            // Fallback to msgpack-lite
                            encodedMsg = msgpack.encode(kickMsg);
                        }
                        
                        socket.send(encodedMsg);
                    }
                }
                
                // Reset click state and set kick time
                lastClickTarget = { x: null, y: null };
                clickWasOnBall = false;
                lastKickTime = Date.now();
            }
        }
    }
}

// Update ball physics (called at fixed 30 FPS rate)
function updateBall(dt) {
    // Преобразуем BALL_FRICTION для шага в 33.33мс
    // Если раньше физика мяча обновлялась каждые 16.67мс (60 FPS),
    // а теперь каждые 33.33мс (30 FPS), то нужно корректировать затухание
    const frictionPerFrame = Math.pow(BALL_FRICTION, dt * 60); // Нормализуем по отношению к 60fps
    
    // Update ball position based on velocity
    ball.logical.x += ball.logical.vx * dt * 60; // Нормализуем скорость по отношению к 60fps
    ball.logical.y += ball.logical.vy * dt * 60;
    
    // Apply friction to slow down the ball
    ball.logical.vx *= frictionPerFrame;
    ball.logical.vy *= frictionPerFrame;
    
    // Constrain ball within canvas boundaries
    const minX = BALL_RADIUS;
    const minY = BALL_RADIUS;
    const maxX = canvas.width - BALL_RADIUS;
    const maxY = canvas.height - BALL_RADIUS;
    
    if (ball.logical.x < minX) {
        ball.logical.x = minX;
        ball.logical.vx = -ball.logical.vx * 0.5; // Bounce with energy loss
    } else if (ball.logical.x > maxX) {
        ball.logical.x = maxX;
        ball.logical.vx = -ball.logical.vx * 0.5; // Bounce with energy loss
    }
    
    if (ball.logical.y < minY) {
        ball.logical.y = minY;
        ball.logical.vy = -ball.logical.vy * 0.5; // Bounce with energy loss
    } else if (ball.logical.y > maxY) {
        ball.logical.y = maxY;
        ball.logical.vy = -ball.logical.vy * 0.5; // Bounce with energy loss
    }
}

// Draw game state - now uses visual positions for rendering
function draw() {
    // Clear canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    
    // Draw field
    ctx.drawImage(fieldImg, 0, 0, canvas.width, canvas.height);
    
    // Draw debug info
    ctx.font = "14px Arial";
    ctx.fillStyle = "black";
    ctx.textAlign = "left";
    ctx.fillText(`Players: ${Object.keys(players).length}`, 10, 20);
    ctx.fillText(`Your ID: ${playerId !== null && playerId !== undefined ? playerId : 'undefined'}`, 10, 40);
    ctx.fillText(`WASM MessagePack: ${wasmReady ? 'Enabled' : 'Disabled'}`, 10, 60);
    
    // НОВОЕ: Отладочная информация о мяче
    if (DEBUG) {
        ctx.fillText(`Ball: x=${Math.round(ball.logical.x)}, y=${Math.round(ball.logical.y)}`, 10, 80);
        ctx.fillText(`Ball vel: vx=${ball.logical.vx.toFixed(2)}, vy=${ball.logical.vy.toFixed(2)}`, 10, 100);
    }
    
    // Draw players
    for (let id in players) {
        const p = players[id];
        
        // Пропустить игроков без корректных координат
        if (!p.visual) continue;
        
        // Now using visual position for rendering
        const x = p.visual.x;
        const y = p.visual.y;
        
        // Highlight local player
        if (id == playerId) {
            ctx.beginPath();
            ctx.arc(x, y, AVATAR_RADIUS + 5, 0, Math.PI * 2);
            ctx.fillStyle = "rgba(255, 255, 0, 0.3)";
            ctx.fill();
        }
        
        // Draw player avatar
        ctx.drawImage(
            avatarImg,
            x - AVATAR_RADIUS,
            y - AVATAR_RADIUS,
            AVATAR_RADIUS * 2,
            AVATAR_RADIUS * 2
        );
        
        // Draw player ID
        ctx.font = "12px Arial";
        ctx.fillStyle = "white";
        ctx.textAlign = "center";
        ctx.fillText(`Player ${id}`, x, y + 5);
        
        // Draw velocity vector for debugging
        if (DEBUG && p.logical && (p.logical.vel_x !== 0 || p.logical.vel_y !== 0)) {
            ctx.beginPath();
            ctx.moveTo(x, y);
            ctx.lineTo(x + p.logical.vel_x * 0.5, y + p.logical.vel_y * 0.5);
            ctx.strokeStyle = "red";
            ctx.stroke();
        }
    }
    
    // Draw ball
    ctx.drawImage(
        ballImg,
        ball.visual.x - BALL_RADIUS,
        ball.visual.y - BALL_RADIUS,
        BALL_RADIUS * 2,
        BALL_RADIUS * 2
    );
    
    // НОВОЕ: Отображение вектора скорости мяча для отладки
    if (DEBUG && (ball.logical.vx !== 0 || ball.logical.vy !== 0)) {
        ctx.beginPath();
        ctx.moveTo(ball.visual.x, ball.visual.y);
        ctx.lineTo(ball.visual.x + ball.logical.vx * 0.5, ball.visual.y + ball.logical.vy * 0.5);
        ctx.strokeStyle = "blue";
        ctx.lineWidth = 2;
        ctx.stroke();
        ctx.lineWidth = 1;
    }
}

// Start the game when window is loaded
window.onload = init;

// Clean up game resources when page is unloaded
window.onunload = () => {
    if (fixedUpdateInterval) {
        clearInterval(fixedUpdateInterval);
    }
    
    if (socket) {
        socket.close();
    }
};