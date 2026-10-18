@echo off
rem play-ffh-rs.cmd: build ffh-play and start the ffh2-rs human game in your browser (Windows).
rem
rem Double-click this file in Explorer, or run it from a terminal. It builds
rem projects\ffh2-rs's ffh-play crate (needs Rust/cargo on PATH) and runs
rem `ffh-play serve`, which hosts the browser client on http://127.0.0.1:8642/.
rem Double-clicking (no arguments) opens the page in your browser once the server
rem is up. --3d opens the experimental 3D map (plain preset; needs a GPU-accelerated
rem browser -- without one the client shows a message and uses the 2D map). Any
rem other arguments are passed through to `ffh-play serve` (--seed, --world, --new,
rem --load, --port, --bind, ...); with pass-through arguments the browser is not
rem opened for you, but the server prints its real URL. On macOS/Linux use
rem play-ffh-rs. Stop by closing this window or pressing Ctrl-C.
setlocal
set "PROJ=%~dp0projects\ffh2-rs"

if not exist "%PROJ%\Cargo.toml" (echo play-ffh-rs: cannot find projects\ffh2-rs next to this script. & pause & exit /b 1)

rem Split arguments: --3d is ours (picks the 3D renderer); everything else goes to serve.
set "THREED="
set "SERVEARGS="
set "HASSERVE="
:parse
if "%~1"=="" goto :parsed
if /i "%~1"=="--3d" (set "THREED=1") else (set "SERVEARGS=%SERVEARGS% %1" & set "HASSERVE=1")
shift
goto :parse
:parsed

set "OPENURL=http://127.0.0.1:8642/"
if defined THREED set "OPENURL=http://127.0.0.1:8642/?renderer=3d&r3d=plain"

cd /d "%PROJ%"

echo play-ffh-rs: building ffh-play (release)...
cargo build --release -p ffh-play
if errorlevel 1 (echo play-ffh-rs: build failed. Is Rust/cargo installed and on PATH? & pause & exit /b 1)
if not exist "target\release\ffh-play.exe" (echo play-ffh-rs: target\release\ffh-play.exe not found after build. & pause & exit /b 1)

if defined THREED echo play-ffh-rs: 3D needs a GPU-accelerated browser; without one it shows a message and uses the 2D map.

if defined HASSERVE goto :noopen
echo play-ffh-rs: your browser will open at "%OPENURL%" when the server is ready.
start "" powershell -NoProfile -WindowStyle Hidden -Command "for($i=0;$i -lt 150;$i++){try{$c=New-Object Net.Sockets.TcpClient;$c.Connect('127.0.0.1',8642);$c.Close();Start-Process '%OPENURL%';break}catch{Start-Sleep -Milliseconds 200}}"
goto :serve
:noopen
echo play-ffh-rs: arguments given; not auto-opening a browser. The server will print its URL below.

:serve
echo play-ffh-rs: starting the game server. Close this window or press Ctrl-C to stop.
target\release\ffh-play.exe serve%SERVEARGS%
pause
