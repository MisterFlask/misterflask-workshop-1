# FFH Simplified

A simplified Fall From Heaven-style 4X strategy game built with TypeScript and HTML5 Canvas.

## Quick Start

```bash
# Install dependencies
npm install

# Start development server
npm run dev

# Build for production
npm run build
```

## Project Structure

```
ffh-simplified/
├── src/
│   ├── main.ts              # Entry point
│   ├── types.ts             # Core type definitions
│   ├── game/                # Game logic
│   │   ├── Game.ts          # State management & actions
│   │   ├── Combat.ts        # OB64-style combat resolution
│   │   └── MapGenerator.ts  # Procedural map generation
│   ├── rendering/
│   │   └── Renderer.ts      # Canvas rendering
│   ├── data/                # Data definitions
│   │   ├── soldiers.ts      # Unit stats
│   │   ├── buildings.ts     # Building definitions
│   │   └── factions.ts      # AI faction config
│   └── utils/
│       ├── grid.ts          # Pathfinding, coordinates
│       └── random.ts        # Seeded RNG
├── assets/
│   ├── sprites/             # Generated PNG sprites
│   └── sprite_definitions/  # Sprite source files (.sprite)
├── tools/
│   └── generate_sprites.py  # Sprite generation script
└── index.html
```

## Controls

- **Click** on a tile to select a legion or city
- **Click** on a highlighted tile to move selected legion
- **Shift+Drag** or **Middle-mouse drag** to pan the camera
- **Enter/Space** to end turn
- **Escape** to deselect

## Design Documents

See the design documentation in `../ffh-simplified-design/`:
- [**Master Design Document**](../../ffh-simplified-design/FFH_SIMPLIFIED_MASTER_DESIGN.md) - Complete game design (factions, units, buildings, magic, combat, AI, balance)
- [Technical Design](../../ffh-simplified-design/technical-design.md) - Implementation architecture and code structure

## Generating Sprites

Sprites are defined in text format and generated to PNG:

```bash
# Requires Python 3 and Pillow
pip install Pillow
python3 tools/compose_all.py   # regenerate .sprite definitions from tools/compose/
npm run generate-sprites       # render .sprite definitions to assets/sprites/*.png
```

Most `.sprite` definitions are composed programmatically by the modules in
`tools/compose/` (drawing helpers live in `tools/sprite_lib.py`); the four
original soldier sprites (fighter, archer, cleric, mage) are hand-authored
and never overwritten. `tools/make_contact_sheet.py <subdir> --out sheet.png`
renders an upscaled review sheet.

## Current Status

**Implemented:**
- Square grid map with terrain
- Player legion movement and selection
- Basic combat resolution (OB64-style)
- Cities and city capture
- Turn structure with end-turn processing
- Gold income and resource tracking
- Armageddon counter

**TODO:**
- AI faction behaviors (state machines per faction type)
- Building construction UI
- Soldier recruitment UI
- Hero system
- Magic/spell system
- Boss faction spawn at Armageddon 80
- Siege mechanics
