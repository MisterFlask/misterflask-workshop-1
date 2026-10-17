@echo off
rem play-ffh-rs.cmd: build ffh-play and start the ffh2-rs human game in your browser (Windows).
rem
rem Double-click this file in Explorer, or run it from a terminal. It builds
rem projects\ffh2-rs's ffh-play crate (needs Rust/cargo on PATH) and runs
rem `ffh-play serve`, which hosts the browser client on http://127.0.0.1:8642/.
rem Double-clicking (no arguments) opens the page in your browser once the server
rem is up. Any arguments are passed through to `ffh-play serve` (--seed, --world,
rem --new, --load, --port, --bind, ...); with arguments the browser is not opened
rem for you, but the server prints its real URL. On macOS/Linux use play-ffh-rs.
rem Stop the game by closing this window or pressing Ctrl-C.
setlocal
set "PROJ=%~dp0projects\ffh2-rs"

if not exist "%PROJ%\Cargo.toml" (echo play-ffh-rs: cannot find projects\ffh2-rs next to this script. & pause & exit /b 1)

cd /d "%PROJ%"

echo play-ffh-rs: building ffh-play (release)...
cargo build --release -p ffh-play
if errorlevel 1 (echo play-ffh-rs: build failed. Is Rust/cargo installed and on PATH? & pause & exit /b 1)
if not exist "target\release\ffh-play.exe" (echo play-ffh-rs: target\release\ffh-play.exe not found after build. & pause & exit /b 1)

if not "%~1"=="" goto :noopen
echo play-ffh-rs: your browser will open at http://127.0.0.1:8642/ when the server is ready.
start "" powershell -NoProfile -WindowStyle Hidden -Command "for($i=0;$i -lt 150;$i++){try{$c=New-Object Net.Sockets.TcpClient;$c.Connect('127.0.0.1',8642);$c.Close();Start-Process 'http://127.0.0.1:8642/';break}catch{Start-Sleep -Milliseconds 200}}"
goto :serve
:noopen
echo play-ffh-rs: arguments given; not auto-opening a browser. The server will print its URL below.

:serve
echo play-ffh-rs: starting the game server. Close this window or press Ctrl-C to stop.
target\release\ffh-play.exe serve %*
pause
