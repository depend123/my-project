#!/bin/bash
set -e

# Цвета для вывода
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}╔══════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║                                                          ║${NC}"
echo -e "${BLUE}║  ${GREEN}Установка York Ball Game с WebAssembly${BLUE}                 ║${NC}"
echo -e "${BLUE}║                                                          ║${NC}"
echo -e "${BLUE}╚══════════════════════════════════════════════════════════╝${NC}"

# Определение директорий проекта и рабочей директории
REPO_URL="https://github.com/depend123/my-project.git"
WORK_DIR="$HOME/york-ball-game"
REPO_DIR="$WORK_DIR/repo"
WEB_DIR="$WORK_DIR/web"
WASM_DIR="$WEB_DIR/wasm"
MSGPACK_WASM_DIR="$WORK_DIR/msgpack_wasm"

echo -e "${YELLOW}1. Создание рабочих директорий${NC}"
mkdir -p $WORK_DIR $WEB_DIR $WASM_DIR $MSGPACK_WASM_DIR/src

echo -e "${YELLOW}2. Проверка и установка необходимых зависимостей${NC}"
# Установка системных зависимостей
if ! dpkg -l | grep -q curl; then
    echo -e "   ${BLUE}Установка curl...${NC}"
    sudo apt-get update
    sudo apt-get install -y curl
fi

# Проверка и установка Rust
if ! command -v rustc &> /dev/null; then
    echo -e "   ${BLUE}Установка Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo -e "   ${GREEN}Rust уже установлен${NC}"
fi

# Проверка и установка wasm-pack
if ! command -v wasm-pack &> /dev/null; then
    echo -e "   ${BLUE}Установка wasm-pack...${NC}"
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
else
    echo -e "   ${GREEN}wasm-pack уже установлен${NC}"
fi

# Клонирование репозитория
echo -e "${YELLOW}3. Клонирование репозитория${NC}"
if [ -d "$REPO_DIR" ]; then
    echo -e "   ${BLUE}Обновление существующего репозитория...${NC}"
    cd $REPO_DIR
    git pull
else
    echo -e "   ${BLUE}Клонирование нового репозитория...${NC}"
    git clone $REPO_URL $REPO_DIR
fi

# Анализ структуры репозитория
echo -e "${YELLOW}4. Анализ структуры проекта${NC}"
cd $REPO_DIR

# Поиск директории с Cargo.toml для msgpack_wasm
MSGPACK_CARGO_TOML=$(find . -name "Cargo.toml" | grep -i msgpack)
if [ -z "$MSGPACK_CARGO_TOML" ]; then
    # Если не найден, возьмем любой Cargo.toml в репозитории
    MSGPACK_CARGO_TOML=$(find . -name "Cargo.toml" | head -1)
fi

if [ -z "$MSGPACK_CARGO_TOML" ]; then
    echo -e "   ${RED}Не найден файл Cargo.toml для WebAssembly модуля!${NC}"
    # Создаем Cargo.toml для msgpack_wasm
    echo -e "   ${BLUE}Создание Cargo.toml для WebAssembly модуля...${NC}"
    cat > "$MSGPACK_WASM_DIR/Cargo.toml" << 'EOF'
[package]
name = "msgpack_wasm"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wasm-bindgen = "0.2"
js-sys = "0.3"
rmp = "0.8"
rmp-serde = "1.1.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
EOF
else
    echo -e "   ${GREEN}Найден Cargo.toml: $MSGPACK_CARGO_TOML${NC}"
    # Копируем Cargo.toml
    cp "$REPO_DIR/$MSGPACK_CARGO_TOML" "$MSGPACK_WASM_DIR/Cargo.toml"
fi

# Поиск lib.rs для msgpack_wasm
LIB_RS=$(find . -name "lib.rs" | grep -i msgpack)
if [ -z "$LIB_RS" ]; then
    # Если не найден, ищем любой lib.rs
    LIB_RS=$(find . -name "lib.rs" | head -1)
fi

if [ -z "$LIB_RS" ]; then
    echo -e "   ${RED}Не найден файл lib.rs для WebAssembly модуля!${NC}"
    
    # Используем директорию с предыдущими файлами, если она есть
    if [ -f "$REPO_DIR/lib.rs" ]; then
        cp "$REPO_DIR/lib.rs" "$MSGPACK_WASM_DIR/src/lib.rs"
        echo -e "   ${GREEN}Найден и скопирован файл lib.rs из корня репозитория${NC}"
    else
        echo -e "   ${RED}ОШИБКА: Не удалось найти файл lib.rs для WebAssembly модуля!${NC}"
        echo -e "   ${YELLOW}Убедитесь, что файл lib.rs присутствует в репозитории.${NC}"
        exit 1
    fi
else
    echo -e "   ${GREEN}Найден lib.rs: $LIB_RS${NC}"
    # Копируем lib.rs
    cp "$REPO_DIR/$LIB_RS" "$MSGPACK_WASM_DIR/src/lib.rs"
fi

# Поиск HTML файла
HTML_FILE=$(find . -name "*.html" | head -1)
if [ -z "$HTML_FILE" ]; then
    echo -e "   ${BLUE}Создание HTML файла...${NC}"
    cat > "$WEB_DIR/index.html" << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>York - Ball Game with WebAssembly</title>
    <style>
        body {
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            align-items: center;
            height: 100vh;
            background-color: #f0f0f0;
        }
        
        #game-container {
            position: relative;
        }
        
        canvas {
            border: 1px solid #333;
            background-color: #fff;
        }
        
        #loading-overlay {
            position: absolute;
            top: 0;
            left: 0;
            width: 100%;
            height: 100%;
            background-color: rgba(0, 0, 0, 0.7);
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            z-index: 10;
            color: white;
            font-family: Arial, sans-serif;
        }
        
        .loader {
            border: 5px solid #f3f3f3;
            border-top: 5px solid #3498db;
            border-radius: 50%;
            width: 50px;
            height: 50px;
            animation: spin 2s linear infinite;
            margin-bottom: 15px;
        }
        
        #loading-text {
            font-size: 18px;
            margin-top: 10px;
        }
        
        #wasm-status {
            font-size: 14px;
            margin-top: 5px;
            padding: 5px 10px;
            border-radius: 4px;
        }
        
        .status-enabled {
            background-color: #4CAF50;
        }
        
        .status-disabled {
            background-color: #f44336;
        }
        
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    </style>
    <!-- Fallback to msgpack-lite if WASM fails to load -->
    <script src="https://cdn.jsdelivr.net/npm/msgpack-lite@0.1.26/dist/msgpack.min.js"></script>
</head>
<body>
    <div id="game-container">
        <canvas id="game-canvas" width="800" height="600"></canvas>
        <div id="loading-overlay">
            <div class="loader"></div>
            <div id="loading-text">Initializing...</div>
            <div id="wasm-status">WebAssembly: Checking support...</div>
        </div>
    </div>

    <script type="text/javascript">
        // Check if the browser supports WebAssembly
        if (typeof WebAssembly === 'object' && typeof WebAssembly.instantiate === 'function') {
            console.log("WebAssembly is supported");
            document.getElementById('wasm-status').textContent = "WebAssembly: Supported";
            document.getElementById('wasm-status').className = "status-enabled";
            
            // We'll update the WebAssembly status when it's ready
            window.addEventListener('wasmReady', () => {
                if (window.wasmReady) {
                    document.getElementById('wasm-status').textContent = "WebAssembly: Enabled";
                    document.getElementById('wasm-status').className = "status-enabled";
                } else {
                    document.getElementById('wasm-status').textContent = "WebAssembly: Fallback mode";
                    document.getElementById('wasm-status').className = "status-disabled";
                }
            }, { once: false });
            
            // Load the client script
            const script = document.createElement('script');
            script.src = './client.js';
            script.type = 'text/javascript';
            document.body.appendChild(script);
        } else {
            console.warn("WebAssembly is not supported in this browser");
            document.getElementById('loading-text').textContent = 
                "Your browser doesn't support WebAssembly";
            document.getElementById('wasm-status').textContent = "WebAssembly: Not supported";
            document.getElementById('wasm-status').className = "status-disabled";
        }
    </script>
</body>
</html>
EOF
else
    echo -e "   ${GREEN}Найден HTML файл: $HTML_FILE${NC}"
    # Копируем HTML файл
    cp "$REPO_DIR/$HTML_FILE" "$WEB_DIR/index.html"
fi

# Поиск JavaScript файлов клиента
CLIENT_JS=$(find . -name "client*.js" | head -1)
if [ -z "$CLIENT_JS" ]; then
    echo -e "   ${RED}Не найден JavaScript файл клиента!${NC}"
    echo -e "   ${YELLOW}Ищем JavaScript файлы в репозитории...${NC}"
    
    # Поиск любых JavaScript файлов
    JS_FILES=$(find . -name "*.js")
    
    if [ -z "$JS_FILES" ]; then
        echo -e "   ${RED}ОШИБКА: Не найдены JavaScript файлы!${NC}"
        exit 1
    else
        # Используем первый найденный JavaScript файл
        FIRST_JS=$(echo "$JS_FILES" | head -1)
        echo -e "   ${BLUE}Использование JavaScript файла: $FIRST_JS${NC}"
        cp "$REPO_DIR/$FIRST_JS" "$WEB_DIR/client.js"
    fi
else
    echo -e "   ${GREEN}Найден JavaScript клиент: $CLIENT_JS${NC}"
    # Копируем JavaScript файл клиента
    cp "$REPO_DIR/$CLIENT_JS" "$WEB_DIR/client.js"
fi

# Компиляция WebAssembly модуля
echo -e "${YELLOW}5. Компиляция WebAssembly модуля${NC}"
cd $MSGPACK_WASM_DIR
source "$HOME/.cargo/env"  # Обеспечиваем доступность команд Rust

# Проверяем, есть ли старая сборка
if [ -d "$MSGPACK_WASM_DIR/pkg" ]; then
    echo -e "   ${BLUE}Удаление старой сборки...${NC}"
    rm -rf "$MSGPACK_WASM_DIR/pkg"
fi

echo -e "   ${BLUE}Запуск wasm-pack...${NC}"
wasm-pack build --target web --out-dir "$WASM_DIR"

if [ $? -ne 0 ]; then
    echo -e "   ${RED}ОШИБКА при компиляции WebAssembly модуля!${NC}"
    echo -e "   ${YELLOW}Продолжаем с отключенным WebAssembly (будет использован msgpack-lite).${NC}"
else
    echo -e "   ${GREEN}WebAssembly модуль успешно скомпилирован!${NC}"
fi

# Запуск веб-сервера
echo -e "${YELLOW}6. Настройка и запуск веб-сервера${NC}"

# Проверка наличия Python
if command -v python3 &> /dev/null; then
    echo -e "   ${BLUE}Запуск веб-сервера с помощью Python...${NC}"
    echo -e "   ${GREEN}Веб-сервер запускается на порту 8000${NC}"
    echo -e "   ${GREEN}Откройте в браузере: http://localhost:8000${NC}"
    echo -e "   ${YELLOW}Нажмите Ctrl+C для завершения работы${NC}"
    
    cd $WEB_DIR
    python3 -m http.server 8000
else
    echo -e "   ${RED}Python не найден для запуска веб-сервера!${NC}"
    echo -e "   ${YELLOW}Установите Python или запустите веб-сервер вручную.${NC}"
    echo -e "   ${YELLOW}Проект настроен в директории: $WEB_DIR${NC}"
fi