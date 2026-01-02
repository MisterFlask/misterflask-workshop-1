import { createInitialGameState, processAction, getValidMoves, getTile, getLegionAt, getCityAt } from './game/Game';
import { Renderer } from './rendering/Renderer';
import { CombatScene } from './rendering/CombatScene';
import { SOLDIER_TYPES } from './data/soldiers';
import { BUILDING_TYPES, getBuildingSlots } from './data/buildings';
import type { GameState, Coord, BuildingId, SoldierTypeId } from './types';

class GameApp {
  private state: GameState;
  private renderer: Renderer;
  private canvas: HTMLCanvasElement;
  private tooltip: HTMLElement;
  private legionPanel: HTMLElement;
  private cityPanel: HTMLElement;
  private unitTooltip: HTMLElement;
  private isDragging = false;
  private lastMousePos = { x: 0, y: 0 };
  private combatScene: CombatScene | null = null;
  private combatCanvas: HTMLCanvasElement | null = null;
  private editingLegion = false;
  private selectedSoldierId: string | null = null;

  constructor() {
    this.canvas = document.getElementById('game-canvas') as HTMLCanvasElement;
    this.tooltip = document.getElementById('tooltip') as HTMLElement;
    this.legionPanel = document.getElementById('legion-panel') as HTMLElement;
    this.cityPanel = document.getElementById('city-panel') as HTMLElement;
    this.unitTooltip = document.getElementById('unit-tooltip') as HTMLElement;
    this.renderer = new Renderer(this.canvas);
    this.state = createInitialGameState();

    this.setupEventListeners();
    this.updateUI();
    this.gameLoop();

    console.log('FFH Simplified initialized!');
    console.log('State:', this.state);
  }

  private setupEventListeners(): void {
    // Canvas mouse events
    this.canvas.addEventListener('mousedown', (e) => this.onMouseDown(e));
    this.canvas.addEventListener('mousemove', (e) => this.onMouseMove(e));
    this.canvas.addEventListener('mouseup', () => this.onMouseUp());
    this.canvas.addEventListener('mouseleave', () => { this.onMouseUp(); this.hideTooltip(); });
    this.canvas.addEventListener('click', (e) => this.onClick(e));

    // End turn button
    const endTurnBtn = document.getElementById('end-turn-btn');
    endTurnBtn?.addEventListener('click', () => this.endTurn());

    // Keyboard
    document.addEventListener('keydown', (e) => this.onKeyDown(e));
  }

  private onMouseDown(e: MouseEvent): void {
    if (e.button === 1 || (e.button === 0 && e.shiftKey)) {
      // Middle mouse or shift+left for panning
      this.isDragging = true;
      this.lastMousePos = { x: e.clientX, y: e.clientY };
    }
  }

  private onMouseMove(e: MouseEvent): void {
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (this.isDragging) {
      const dx = e.clientX - this.lastMousePos.x;
      const dy = e.clientY - this.lastMousePos.y;
      this.renderer.setCamera(
        this.renderer['uiState'].camera.x - dx,
        this.renderer['uiState'].camera.y - dy
      );
      this.lastMousePos = { x: e.clientX, y: e.clientY };
      this.hideTooltip();
    } else {
      const worldPos = this.renderer.screenToWorld(x, y);
      this.renderer.setHoveredTile(worldPos);
      this.updateTooltip(worldPos, e.clientX, e.clientY);
    }
  }

  private updateTooltip(coord: Coord, mouseX: number, mouseY: number): void {
    // Check if coord is valid
    if (
      coord.x < 0 ||
      coord.x >= this.state.mapWidth ||
      coord.y < 0 ||
      coord.y >= this.state.mapHeight
    ) {
      this.hideTooltip();
      return;
    }

    const tile = getTile(this.state, coord);
    const legion = getLegionAt(this.state, coord);
    const city = getCityAt(this.state, coord);

    if (!tile) {
      this.hideTooltip();
      return;
    }

    let html = '';

    // Terrain info
    const terrainNames: Record<string, string> = {
      grass: 'Grassland',
      forest: 'Forest (+10% defense)',
      hills: 'Hills (+15% defense)',
      mountain: 'Mountains (impassable)',
      water: 'Water (impassable)',
    };
    html += `<p class="terrain">${terrainNames[tile.terrain] || tile.terrain}</p>`;

    // City info
    if (city) {
      const isPlayer = city.owner === 'player';
      const ownerClass = isPlayer ? 'friendly' : 'city';
      html += `<h4 class="${ownerClass}">${city.name}</h4>`;
      html += `<p>Owner: ${city.owner}</p>`;
      html += `<p>Population: ${city.population}</p>`;
      if (city.buildings.length > 0) {
        html += `<p>Buildings: ${city.buildings.join(', ')}</p>`;
      }
      if (city.occupationTurns > 0) {
        html += `<p style="color:#f88">Occupied (${city.occupationTurns} turns)</p>`;
      }
      const hasWalls = city.buildings.includes('walls');
      if (hasWalls) {
        html += `<p style="color:#8f8">Walls (+40% defense)</p>`;
      }
    }

    // Legion info
    if (legion) {
      const isPlayer = legion.owner === 'player';
      const ownerClass = isPlayer ? 'friendly' : 'legion';
      html += `<h4 class="${ownerClass}">${isPlayer ? 'Your' : legion.owner} Legion</h4>`;
      html += `<p>Soldiers: ${legion.soldiers.length}/8</p>`;
      if (isPlayer) {
        html += `<p>Movement: ${legion.movementRemaining}/3</p>`;
      }

      // Summarize soldier types
      const soldierCounts: Record<string, number> = {};
      let totalHp = 0;
      let maxHp = 0;
      for (const s of legion.soldiers) {
        soldierCounts[s.type] = (soldierCounts[s.type] || 0) + 1;
        totalHp += s.hp;
        maxHp += s.maxHp;
      }
      const composition = Object.entries(soldierCounts)
        .map(([type, count]) => `${count}x ${type}`)
        .join(', ');
      html += `<p>Composition: ${composition}</p>`;
      html += `<p>Total HP: ${totalHp}/${maxHp}</p>`;
    }

    // Show tooltip
    this.tooltip.innerHTML = html;
    this.tooltip.style.display = 'block';

    // Position tooltip near cursor but keep on screen
    const tooltipRect = this.tooltip.getBoundingClientRect();
    let left = mouseX + 15;
    let top = mouseY + 15;

    if (left + tooltipRect.width > window.innerWidth) {
      left = mouseX - tooltipRect.width - 15;
    }
    if (top + tooltipRect.height > window.innerHeight) {
      top = mouseY - tooltipRect.height - 15;
    }

    this.tooltip.style.left = `${left}px`;
    this.tooltip.style.top = `${top}px`;
  }

  private hideTooltip(): void {
    this.tooltip.style.display = 'none';
  }

  private onMouseUp(): void {
    this.isDragging = false;
  }

  private onClick(e: MouseEvent): void {
    if (this.state.phase !== 'player_turn') return;

    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const worldPos = this.renderer.screenToWorld(x, y);

    // Validate coordinates
    if (
      worldPos.x < 0 ||
      worldPos.x >= this.state.mapWidth ||
      worldPos.y < 0 ||
      worldPos.y >= this.state.mapHeight
    ) {
      return;
    }

    this.state = processAction(this.state, {
      type: 'select_tile',
      coord: worldPos,
    });

    this.updateValidMoves();
    this.updateUI();
    this.checkForCombat();
  }

  private onKeyDown(e: KeyboardEvent): void {
    if (e.key === 'Enter' || e.key === ' ') {
      this.endTurn();
    }
    if (e.key === 'Escape') {
      this.state = { ...this.state, selectedLegionId: null, selectedCityId: null };
      this.updateValidMoves();
      this.updateUI();
    }
  }

  private endTurn(): void {
    if (this.state.phase !== 'player_turn') return;

    this.state = processAction(this.state, { type: 'end_turn' });
    this.updateUI();
    this.updateValidMoves();
  }

  private updateValidMoves(): void {
    if (this.state.selectedLegionId) {
      const legion = this.state.legions.get(this.state.selectedLegionId);
      if (legion && legion.owner === 'player') {
        this.renderer.setValidMoves(getValidMoves(this.state, legion));
        return;
      }
    }
    this.renderer.setValidMoves([]);
  }

  private updateUI(): void {
    const player = this.state.factions.get('player');

    // Update resource display
    const goldEl = document.getElementById('gold-amount');
    const manaEl = document.getElementById('mana-amount');
    const turnEl = document.getElementById('turn-indicator');
    const armageddonEl = document.getElementById('armageddon-value');
    const armageddonFill = document.getElementById('armageddon-fill');

    if (goldEl && player) goldEl.textContent = player.gold.toString();
    if (manaEl && player) manaEl.textContent = player.mana.toString();
    if (turnEl) turnEl.textContent = `Turn ${this.state.turn}`;
    if (armageddonEl) armageddonEl.textContent = `${this.state.armageddonCounter}/100`;
    if (armageddonFill) armageddonFill.style.width = `${this.state.armageddonCounter}%`;

    // Update selection panel
    this.updateSelectionPanel();

    // Update legion panel
    this.updateLegionPanel();

    // Update city panel
    this.updateCityPanel();

    // Game over check
    if (this.state.gameOver) {
      const message = this.state.winner === 'player'
        ? 'Victory! You have defeated the Infernal Legion!'
        : 'Defeat! Your empire has fallen.';
      alert(message);
    }
  }

  private updateSelectionPanel(): void {
    const panel = document.getElementById('selection-panel');
    if (!panel) return;

    if (this.state.selectedCityId) {
      const city = this.state.cities.get(this.state.selectedCityId);
      if (city) {
        panel.style.display = 'block';
        panel.innerHTML = `
          <h3>${city.name}</h3>
          <p>Population: ${city.population}</p>
          <p>Buildings: ${city.buildings.length > 0 ? city.buildings.join(', ') : 'None'}</p>
          ${city.occupationTurns > 0 ? `<p style="color: #f88">Occupied (${city.occupationTurns} turns)</p>` : ''}
        `;
        return;
      }
    }

    if (this.state.selectedLegionId) {
      const legion = this.state.legions.get(this.state.selectedLegionId);
      if (legion) {
        panel.style.display = 'block';
        panel.innerHTML = `
          <h3>Legion</h3>
          <p>Soldiers: ${legion.soldiers.length}/8</p>
          <p>Movement: ${legion.movementRemaining}/3</p>
          <p>Composition:</p>
          <ul style="margin-left: 16px; font-size: 12px">
            ${legion.soldiers.map(s => `<li>${s.type} (${s.hp}/${s.maxHp} HP)</li>`).join('')}
          </ul>
        `;
        return;
      }
    }

    panel.style.display = 'none';
  }

  private updateLegionPanel(): void {
    if (!this.state.selectedLegionId) {
      this.legionPanel.style.display = 'none';
      this.editingLegion = false;
      this.selectedSoldierId = null;
      return;
    }

    const legion = this.state.legions.get(this.state.selectedLegionId);
    if (!legion) {
      this.legionPanel.style.display = 'none';
      this.editingLegion = false;
      this.selectedSoldierId = null;
      return;
    }

    const isPlayer = legion.owner === 'player';
    const ownerColor = isPlayer ? '#4af' : '#f44';
    const ownerLabel = isPlayer ? 'Your Legion' : `${legion.owner} Legion`;

    // Calculate total HP
    let totalHp = 0;
    let totalMaxHp = 0;
    for (const s of legion.soldiers) {
      totalHp += s.hp;
      totalMaxHp += s.maxHp;
    }

    // Build a map of position -> soldier for quick lookup
    const positionMap = new Map<string, typeof legion.soldiers[0]>();
    for (const soldier of legion.soldiers) {
      const key = `${soldier.position.row}-${soldier.position.column}`;
      positionMap.set(key, soldier);
    }

    // Helper to render a cell
    const renderCell = (row: 'front' | 'mid' | 'back', col: number): string => {
      const soldier = positionMap.get(`${row}-${col}`);
      const posKey = `${row}-${col}`;
      const editClass = this.editingLegion && isPlayer ? 'edit-mode' : '';
      const isSelected = soldier && soldier.id === this.selectedSoldierId;
      const selectedClass = isSelected ? 'selected' : '';

      if (!soldier) {
        return `<div class="formation-cell ${editClass}" data-pos="${posKey}"></div>`;
      }

      const hpPercent = (soldier.hp / soldier.maxHp) * 100;
      const hpClass = hpPercent > 50 ? 'high' : hpPercent > 25 ? 'medium' : 'low';
      const isDead = soldier.hp <= 0;

      return `
        <div class="formation-cell occupied ${editClass} ${selectedClass}" data-soldier-id="${soldier.id}" data-pos="${posKey}">
          <div class="soldier-card ${isDead ? 'dead' : ''}">
            <div class="soldier-icon ${soldier.type}">${soldier.type.slice(0, 3).toUpperCase()}</div>
            <div class="soldier-hp-bar">
              <div class="soldier-hp-fill ${hpClass}" style="width: ${Math.max(0, hpPercent)}%"></div>
            </div>
            <div class="soldier-hp-text">${Math.max(0, soldier.hp)}/${soldier.maxHp}</div>
          </div>
        </div>
      `;
    };

    // Build the formation grid
    const rows: Array<'front' | 'mid' | 'back'> = ['front', 'mid', 'back'];
    const formationGrid = rows.map(row => `
      <div class="formation-row">
        <div class="formation-row-label ${row}">${row}</div>
        <div class="formation-cells">
          ${renderCell(row, 0)}
          ${renderCell(row, 1)}
          ${renderCell(row, 2)}
        </div>
      </div>
    `).join('');

    // Build actions section
    let actionsHtml = '';
    if (isPlayer && legion.soldiers.length > 0) {
      if (this.editingLegion) {
        actionsHtml = `
          <div class="edit-instructions">
            Click a soldier to select, then click another position to move/swap
          </div>
          <div class="legion-actions">
            <button class="legion-btn primary" id="done-edit-btn">Done</button>
            <button class="legion-btn cancel" id="cancel-edit-btn">Cancel</button>
          </div>
        `;
      } else {
        actionsHtml = `
          <div class="legion-actions">
            <button class="legion-btn" id="edit-legion-btn">Edit Formation</button>
          </div>
        `;
      }
    }

    this.legionPanel.innerHTML = `
      <div class="legion-header">
        <h3 style="color: ${ownerColor}; border: none; margin: 0; padding: 0;">${ownerLabel}</h3>
      </div>
      <div class="legion-stats">
        ${legion.soldiers.length}/8 soldiers | ${totalHp}/${totalMaxHp} HP
        ${isPlayer ? ` | ${legion.movementRemaining}/3 move` : ''}
      </div>
      <div class="formation-grid">
        ${formationGrid}
      </div>
      ${actionsHtml}
    `;

    this.legionPanel.style.display = 'block';

    // Set up event listeners
    if (this.editingLegion && isPlayer) {
      this.setupEditModeListeners(legion);
    } else {
      this.setupSoldierTooltips(legion);
    }

    // Set up button listeners
    const editBtn = document.getElementById('edit-legion-btn');
    if (editBtn) {
      editBtn.addEventListener('click', () => this.startEditingLegion());
    }

    const doneBtn = document.getElementById('done-edit-btn');
    if (doneBtn) {
      doneBtn.addEventListener('click', () => this.finishEditingLegion());
    }

    const cancelBtn = document.getElementById('cancel-edit-btn');
    if (cancelBtn) {
      cancelBtn.addEventListener('click', () => this.cancelEditingLegion());
    }
  }

  private updateCityPanel(): void {
    if (!this.state.selectedCityId) {
      this.cityPanel.style.display = 'none';
      return;
    }

    const city = this.state.cities.get(this.state.selectedCityId);
    if (!city || city.owner !== 'player') {
      this.cityPanel.style.display = 'none';
      return;
    }

    const player = this.state.factions.get('player');
    if (!player) return;

    const slots = getBuildingSlots(city.population);
    const queuedBuildings = city.buildQueue.filter(item => item.itemType === 'building').length;
    const usedSlots = city.buildings.length + queuedBuildings;

    // Get legion at city for soldier recruitment
    const legionAtCity = getLegionAt(this.state, city.coord);
    const isPlayerLegion = legionAtCity && legionAtCity.owner === 'player';

    // Build queue section
    let queueHtml = '';
    if (city.buildQueue.length > 0) {
      queueHtml = `
        <div class="city-section">
          <div class="city-section-title">Build Queue</div>
          <div class="build-queue">
            ${city.buildQueue.map(item => {
              const isBuilding = item.itemType === 'building';
              const data = isBuilding
                ? BUILDING_TYPES[item.itemId as BuildingId]
                : SOLDIER_TYPES[item.itemId as SoldierTypeId];
              const progressPercent = ((item.totalTurns - item.turnsRemaining) / item.totalTurns) * 100;
              return `
                <div class="queue-item">
                  <div class="queue-item-info">
                    <div class="queue-item-name">${data.name}</div>
                    <div class="queue-item-turns">${item.turnsRemaining} turn${item.turnsRemaining > 1 ? 's' : ''} remaining</div>
                    <div class="queue-progress">
                      <div class="queue-progress-fill" style="width: ${progressPercent}%"></div>
                    </div>
                  </div>
                  <button class="queue-item-cancel" data-queue-id="${item.id}">X</button>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    }

    // Existing buildings section
    let existingBuildingsHtml = '';
    if (city.buildings.length > 0) {
      existingBuildingsHtml = `
        <div class="city-section">
          <div class="city-section-title">Buildings (${city.buildings.length}/${slots})</div>
          <div class="existing-buildings">
            ${city.buildings.map(b => `<span class="existing-building">${BUILDING_TYPES[b].name}</span>`).join('')}
          </div>
        </div>
      `;
    }

    // Available buildings section
    const availableBuildings = (Object.keys(BUILDING_TYPES) as BuildingId[]).filter(bid => {
      if (city.buildings.includes(bid)) return false;
      if (city.buildQueue.some(item => item.itemType === 'building' && item.itemId === bid)) return false;
      return true;
    });

    let buildingsHtml = '';
    if (usedSlots < slots && availableBuildings.length > 0) {
      buildingsHtml = `
        <div class="city-section">
          <div class="city-section-title">Build Building</div>
          <div class="available-items">
            ${availableBuildings.map(bid => {
              const b = BUILDING_TYPES[bid];
              const canAfford = player.gold >= b.cost;
              return `
                <div class="build-option">
                  <div class="build-option-info">
                    <div class="build-option-name">${b.name}</div>
                    <div class="build-option-cost">${b.cost} gold | ${b.buildTurns} turns</div>
                  </div>
                  <button class="build-option-btn" data-build-type="building" data-build-id="${bid}" ${!canAfford ? 'disabled' : ''}>
                    Build
                  </button>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    }

    // Available soldiers section (only if player legion is at city)
    let soldiersHtml = '';
    if (isPlayerLegion && city.occupationTurns === 0) {
      // Determine which soldiers can be recruited
      const recruitableSoldiers: SoldierTypeId[] = ['fighter']; // Always available
      for (const buildingId of city.buildings) {
        const building = BUILDING_TYPES[buildingId];
        for (const effect of building.effects) {
          if (effect.type === 'unlock_soldier' && !recruitableSoldiers.includes(effect.soldier)) {
            recruitableSoldiers.push(effect.soldier);
          }
        }
      }

      // Check legion capacity
      const queuedForLegion = city.buildQueue.filter(
        item => item.itemType === 'soldier' && item.targetLegionId === legionAtCity.id
      ).length;
      const legionFull = legionAtCity.soldiers.length + queuedForLegion >= 8;

      soldiersHtml = `
        <div class="city-section">
          <div class="city-section-title">Recruit Soldier (${legionAtCity.soldiers.length + queuedForLegion}/8)</div>
          <div class="available-items">
            ${recruitableSoldiers.map(sid => {
              const s = SOLDIER_TYPES[sid];
              const canAfford = player.gold >= s.cost.gold && player.mana >= s.cost.mana;
              const costText = s.cost.mana > 0
                ? `${s.cost.gold} gold, ${s.cost.mana} mana`
                : `${s.cost.gold} gold`;
              return `
                <div class="build-option">
                  <div class="build-option-info">
                    <div class="build-option-name">${s.name}</div>
                    <div class="build-option-cost">${costText} | ${s.buildTurns} turn${s.buildTurns > 1 ? 's' : ''}</div>
                  </div>
                  <button class="build-option-btn" data-build-type="soldier" data-build-id="${sid}" ${!canAfford || legionFull ? 'disabled' : ''}>
                    Train
                  </button>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    }

    this.cityPanel.innerHTML = `
      <h3>${city.name}</h3>
      <div class="city-stats">
        <span>Population: ${city.population}</span>
        <span>Building Slots: ${usedSlots}/${slots}</span>
        ${city.occupationTurns > 0 ? `<span style="color: #f88">Occupied (${city.occupationTurns} turns)</span>` : ''}
        ${isPlayerLegion ? `<span style="color: #8cf">Legion Present</span>` : ''}
      </div>
      ${queueHtml}
      ${existingBuildingsHtml}
      ${buildingsHtml}
      ${soldiersHtml}
    `;

    this.cityPanel.style.display = 'block';

    // Set up button listeners
    this.setupCityPanelListeners(city.id, legionAtCity?.id || null);
  }

  private setupCityPanelListeners(cityId: string, legionId: string | null): void {
    // Cancel queue buttons
    const cancelBtns = this.cityPanel.querySelectorAll('.queue-item-cancel');
    cancelBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const queueId = btn.getAttribute('data-queue-id');
        if (queueId) {
          this.state = processAction(this.state, {
            type: 'cancel_queue_item',
            cityId,
            queueItemId: queueId,
          });
          this.updateUI();
        }
      });
    });

    // Build buttons
    const buildBtns = this.cityPanel.querySelectorAll('.build-option-btn');
    buildBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const buildType = btn.getAttribute('data-build-type');
        const buildId = btn.getAttribute('data-build-id');
        if (!buildType || !buildId) return;

        if (buildType === 'building') {
          this.state = processAction(this.state, {
            type: 'queue_building',
            cityId,
            buildingId: buildId as BuildingId,
          });
        } else if (buildType === 'soldier' && legionId) {
          this.state = processAction(this.state, {
            type: 'queue_soldier',
            cityId,
            soldierType: buildId as SoldierTypeId,
            targetLegionId: legionId,
          });
        }
        this.updateUI();
      });
    });
  }

  private startEditingLegion(): void {
    this.editingLegion = true;
    this.selectedSoldierId = null;
    this.updateLegionPanel();
  }

  private finishEditingLegion(): void {
    this.editingLegion = false;
    this.selectedSoldierId = null;
    this.updateLegionPanel();
  }

  private cancelEditingLegion(): void {
    // For now, cancel is the same as done since we apply changes immediately
    // Could implement undo by storing original positions
    this.editingLegion = false;
    this.selectedSoldierId = null;
    this.updateLegionPanel();
  }

  private setupEditModeListeners(legion: typeof this.state.legions extends Map<string, infer T> ? T : never): void {
    const cells = this.legionPanel.querySelectorAll('.formation-cell.edit-mode');

    cells.forEach(cell => {
      cell.addEventListener('click', () => {
        const soldierId = cell.getAttribute('data-soldier-id');
        const posKey = cell.getAttribute('data-pos');

        if (!posKey) return;

        const [row, colStr] = posKey.split('-');
        const col = parseInt(colStr, 10);
        const targetPos = { row: row as 'front' | 'mid' | 'back', column: col };

        if (this.selectedSoldierId) {
          // We have a soldier selected, try to move or swap
          this.moveSoldierToPosition(legion, this.selectedSoldierId, targetPos, soldierId || null);
          this.selectedSoldierId = null;
        } else if (soldierId) {
          // Select this soldier
          this.selectedSoldierId = soldierId;
        }

        this.updateLegionPanel();
      });
    });
  }

  private moveSoldierToPosition(
    legion: typeof this.state.legions extends Map<string, infer T> ? T : never,
    movingSoldierId: string,
    targetPos: { row: 'front' | 'mid' | 'back'; column: number },
    targetSoldierId: string | null
  ): void {
    const movingSoldier = legion.soldiers.find(s => s.id === movingSoldierId);
    if (!movingSoldier) return;

    if (targetSoldierId) {
      // Swap positions with target soldier
      const targetSoldier = legion.soldiers.find(s => s.id === targetSoldierId);
      if (targetSoldier && targetSoldier.id !== movingSoldierId) {
        const tempPos = { ...movingSoldier.position };
        movingSoldier.position = { ...targetSoldier.position };
        targetSoldier.position = tempPos;
      }
    } else {
      // Move to empty position
      movingSoldier.position = targetPos;
    }

    // Update the state (soldiers array is mutated directly, but we need to trigger a re-render)
    const newLegions = new Map(this.state.legions);
    newLegions.set(legion.id, { ...legion });
    this.state = { ...this.state, legions: newLegions };
  }

  private setupSoldierTooltips(legion: typeof this.state.legions extends Map<string, infer T> ? T : never): void {
    const cells = this.legionPanel.querySelectorAll('.formation-cell[data-soldier-id]');

    cells.forEach(cell => {
      const soldierId = cell.getAttribute('data-soldier-id');
      if (!soldierId) return;

      const soldier = legion.soldiers.find(s => s.id === soldierId);
      if (!soldier) return;

      cell.addEventListener('mouseenter', (e) => {
        this.showUnitTooltip(soldier, e as MouseEvent);
      });

      cell.addEventListener('mousemove', (e) => {
        this.positionUnitTooltip(e as MouseEvent);
      });

      cell.addEventListener('mouseleave', () => {
        this.hideUnitTooltip();
      });
    });
  }

  private showUnitTooltip(soldier: typeof this.state.legions extends Map<string, infer T> ? T['soldiers'][0] : never, e: MouseEvent): void {
    const soldierType = SOLDIER_TYPES[soldier.type];
    const hpPercent = (soldier.hp / soldier.maxHp) * 100;
    const hpClass = hpPercent > 50 ? 'hp-high' : hpPercent > 25 ? 'hp-medium' : 'hp-low';

    // Get attacks per row
    const attacksInRow = soldierType.attackCount[soldier.position.row];
    const targetPref = soldierType.attacksTarget === 'front' ? 'Front row first' : 'Back row first';

    this.unitTooltip.innerHTML = `
      <h4 style="color: ${this.getSoldierColor(soldier.type)}">${soldierType.name}</h4>

      <div class="stat-row">
        <span class="stat-label">HP</span>
        <span class="stat-value ${hpClass}">${Math.max(0, soldier.hp)} / ${soldier.maxHp}</span>
      </div>
      <div class="stat-row">
        <span class="stat-label">Position</span>
        <span class="stat-value">${soldier.position.row} row, col ${soldier.position.column + 1}</span>
      </div>

      <div class="section-label">Combat Stats</div>
      <div class="stat-row">
        <span class="stat-label">Attack</span>
        <span class="stat-value">${soldierType.attack}</span>
      </div>
      <div class="stat-row">
        <span class="stat-label">Defense</span>
        <span class="stat-value">${soldierType.defense}</span>
      </div>
      <div class="stat-row">
        <span class="stat-label">Speed</span>
        <span class="stat-value">${soldierType.speed}</span>
      </div>

      <div class="section-label">Attacks</div>
      <div class="attack-info">
        <div class="stat-row">
          <span class="stat-label">Attacks/round</span>
          <span class="stat-value">${attacksInRow}</span>
        </div>
        <div class="stat-row">
          <span class="stat-label">Targets</span>
          <span class="stat-value">${targetPref}</span>
        </div>
        <div class="stat-row">
          <span class="stat-label">Preferred row</span>
          <span class="stat-value">${soldierType.preferredRow}</span>
        </div>
      </div>

      <div class="section-label">Attacks by Position</div>
      <div style="font-size: 11px; color: #aaa;">
        Front: ${soldierType.attackCount.front} | Mid: ${soldierType.attackCount.mid} | Back: ${soldierType.attackCount.back}
      </div>
    `;

    this.unitTooltip.style.display = 'block';
    this.positionUnitTooltip(e);
  }

  private positionUnitTooltip(e: MouseEvent): void {
    const tooltipRect = this.unitTooltip.getBoundingClientRect();
    let left = e.clientX + 15;
    let top = e.clientY + 15;

    // Keep on screen
    if (left + tooltipRect.width > window.innerWidth) {
      left = e.clientX - tooltipRect.width - 15;
    }
    if (top + tooltipRect.height > window.innerHeight) {
      top = e.clientY - tooltipRect.height - 15;
    }

    this.unitTooltip.style.left = `${left}px`;
    this.unitTooltip.style.top = `${top}px`;
  }

  private hideUnitTooltip(): void {
    this.unitTooltip.style.display = 'none';
  }

  private getSoldierColor(type: string): string {
    const colors: Record<string, string> = {
      fighter: '#c84',
      knight: '#88c',
      archer: '#4c8',
      mage: '#c4c',
      cleric: '#fff',
      demon: '#c44',
    };
    return colors[type] || '#aaa';
  }

  private showCombatScene(): void {
    if (!this.state.pendingCombat) return;

    // Create combat canvas overlay
    this.combatCanvas = document.createElement('canvas');
    this.combatCanvas.width = 800;
    this.combatCanvas.height = 400;
    this.combatCanvas.style.cssText = `
      position: fixed;
      top: 50%;
      left: 50%;
      transform: translate(-50%, -50%);
      z-index: 1000;
      border: 4px solid #444;
      border-radius: 8px;
      box-shadow: 0 0 40px rgba(0, 0, 0, 0.8);
    `;
    document.body.appendChild(this.combatCanvas);

    // Create backdrop
    const backdrop = document.createElement('div');
    backdrop.id = 'combat-backdrop';
    backdrop.style.cssText = `
      position: fixed;
      top: 0;
      left: 0;
      width: 100%;
      height: 100%;
      background: rgba(0, 0, 0, 0.7);
      z-index: 999;
    `;
    document.body.appendChild(backdrop);

    // Create and start combat scene
    this.combatScene = new CombatScene(
      this.combatCanvas,
      this.state.pendingCombat.result,
      () => this.onCombatComplete()
    );
    this.combatScene.start();
  }

  private onCombatComplete(): void {
    // Clean up combat scene
    if (this.combatCanvas) {
      this.combatCanvas.remove();
      this.combatCanvas = null;
    }
    const backdrop = document.getElementById('combat-backdrop');
    if (backdrop) backdrop.remove();
    this.combatScene = null;

    // Apply combat results to game state
    this.state = processAction(this.state, { type: 'apply_combat_results' });
    this.updateUI();
    this.updateValidMoves();
  }

  private checkForCombat(): void {
    if (this.state.phase === 'combat_resolution' && this.state.pendingCombat && !this.combatScene) {
      this.showCombatScene();
    }
  }

  private gameLoop(): void {
    this.renderer.render(this.state);
    requestAnimationFrame(() => this.gameLoop());
  }
}

// Start the game when DOM is ready
document.addEventListener('DOMContentLoaded', () => {
  new GameApp();
});
