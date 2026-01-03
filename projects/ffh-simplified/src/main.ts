import { createInitialGameState, processAction, getValidMoves, getTile, getLegionAt, getCityAt, getLegionsInRangeOfCity, getCitiesInRangeOfLegion, getLegionsInRangeOfLegion, getLegionName, getFactionIncome, getCityIncome, getCityGrowthInfo, getCityDefenseBonus } from './game/Game';
import { Renderer } from './rendering/Renderer';
import { CombatScene } from './rendering/CombatScene';
import { SOLDIER_TYPES } from './data/soldiers';
import { BUILDING_TYPES, getBuildingSlots } from './data/buildings';
import { TECHNOLOGIES } from './data/technologies';
import type { GameState, Coord, BuildingId, SoldierTypeId } from './types';

class GameApp {
  private state: GameState;
  private renderer: Renderer;
  private canvas: HTMLCanvasElement;
  private tooltip: HTMLElement;
  private legionPanel: HTMLElement;
  private cityPanel: HTMLElement;
  private unitTooltip: HTMLElement;
  private collegiaPanel: HTMLElement;
  private collegiaVisible = false;
  private knowledgePanel: HTMLElement;
  private knowledgeVisible = false;
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
    this.collegiaPanel = document.getElementById('collegia-panel') as HTMLElement;
    this.knowledgePanel = document.getElementById('knowledge-panel') as HTMLElement;
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

    // Research indicator click - open Collegia panel
    const researchIndicator = document.getElementById('research-indicator');
    researchIndicator?.addEventListener('click', () => this.toggleCollegiaPanel());

    // Knowledge indicator click - open Knowledge panel
    const knowledgeIndicator = document.getElementById('knowledge-indicator');
    knowledgeIndicator?.addEventListener('click', () => this.toggleKnowledgePanel());

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
      const capitalLabel = city.isCapital ? ' (Capital)' : '';
      const ownerLabel = isPlayer ? 'Your' : city.owner;
      html += `<h4 class="${ownerClass}">${city.name}</h4>`;
      html += `<p>${ownerLabel} city${capitalLabel}</p>`;
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
    const goldIncomeEl = document.getElementById('gold-income');
    const manaIncomeEl = document.getElementById('mana-income');
    const goldResourceEl = document.getElementById('gold-resource');
    const manaResourceEl = document.getElementById('mana-resource');
    const turnEl = document.getElementById('turn-indicator');
    const armageddonEl = document.getElementById('armageddon-value');
    const armageddonFill = document.getElementById('armageddon-fill');

    if (goldEl && player) goldEl.textContent = player.gold.toString();
    if (manaEl && player) manaEl.textContent = player.mana.toString();

    // Calculate and display income per turn
    const income = getFactionIncome(this.state, 'player');
    if (goldIncomeEl) {
      goldIncomeEl.textContent = `(+${income.totalGold})`;
      goldIncomeEl.className = income.totalGold > 0 ? 'income-indicator' : 'income-indicator zero';
    }
    if (manaIncomeEl) {
      if (income.totalMana > 0) {
        manaIncomeEl.textContent = `(+${income.totalMana})`;
        manaIncomeEl.className = 'income-indicator';
      } else {
        manaIncomeEl.textContent = '';
      }
    }

    // Add tooltips with breakdown
    if (goldResourceEl) {
      const breakdown = income.cityBreakdowns.map(c => `${c.cityName}: +${c.gold}`).join('\n');
      goldResourceEl.title = `Gold per turn: +${income.totalGold}\n${breakdown}`;
    }
    if (manaResourceEl && income.totalMana > 0) {
      const manaBreakdown = income.cityBreakdowns.filter(c => c.mana > 0).map(c => `${c.cityName}: +${c.mana}`).join('\n');
      manaResourceEl.title = `Mana per turn: +${income.totalMana}\n${manaBreakdown}`;
    }

    if (turnEl) turnEl.textContent = `Turn ${this.state.turn}`;
    if (armageddonEl) armageddonEl.textContent = `${this.state.armageddonCounter}/100`;
    if (armageddonFill) armageddonFill.style.width = `${this.state.armageddonCounter}%`;

    // Update research indicator
    this.updateResearchIndicator();

    // Update knowledge count
    this.updateKnowledgeCount();

    // Update selection panel
    this.updateSelectionPanel();

    // Update legion panel
    this.updateLegionPanel();

    // Update city panel
    this.updateCityPanel();

    // Update Collegia panel if visible
    if (this.collegiaVisible) {
      this.updateCollegiaPanel();
    }

    // Update Knowledge panel if visible
    if (this.knowledgeVisible) {
      this.updateKnowledgePanel();
    }

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
        const isPlayer = city.owner === 'player';
        const capitalLabel = city.isCapital ? ' (Capital)' : '';
        const ownerLabel = isPlayer ? 'Your' : city.owner;
        panel.style.display = 'block';
        panel.innerHTML = `
          <h3>${city.name}</h3>
          <p style="color: ${isPlayer ? '#4af' : '#f44'}; font-size: 11px;">${ownerLabel} city${capitalLabel}</p>
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

    // Check for nearby player cities (for roster management)
    const nearbyCities = isPlayer ? getCitiesInRangeOfLegion(this.state, legion, 'player') : [];
    const hasNearbyCities = nearbyCities.length > 0;

    // Check for nearby player legions (for transfers)
    const nearbyLegions = isPlayer ? getLegionsInRangeOfLegion(this.state, legion, 'player') : [];
    const hasNearbyLegions = nearbyLegions.length > 0;

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
      const leaderIndicator = soldier.isLeader ? '<span class="leader-crown" title="Leader">★</span>' : '';

      return `
        <div class="formation-cell occupied ${editClass} ${selectedClass}" data-soldier-id="${soldier.id}" data-pos="${posKey}">
          <div class="soldier-card ${isDead ? 'dead' : ''}">
            ${leaderIndicator}
            <div class="soldier-icon ${soldier.type}">${soldier.type.slice(0, 3).toUpperCase()}</div>
            <div class="soldier-name-label">${soldier.name}</div>
            <div class="soldier-hp-bar">
              <div class="soldier-hp-fill ${hpClass}" style="width: ${Math.max(0, hpPercent)}%"></div>
            </div>
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
        const showTransfers = this.selectedSoldierId;
        // Generate unassign buttons for each nearby city
        const unassignButtons = showTransfers && hasNearbyCities ? nearbyCities.map(city => {
          const distance = Math.abs(city.coord.x - legion.location.x) + Math.abs(city.coord.y - legion.location.y);
          const distLabel = distance === 0 ? 'here' : `${distance} away`;
          const cityLabel = city.name.length > 10 ? city.name.slice(0, 8) + '..' : city.name;
          return `<button class="legion-btn unassign-city-btn" data-city-id="${city.id}" style="background: #5a4a2a;" title="${city.name} (${distLabel})">→${cityLabel}</button>`;
        }).join('') : '';

        // Generate transfer buttons for each nearby legion with room
        const transferButtons = showTransfers && hasNearbyLegions ? nearbyLegions.map(targetLegion => {
          const distance = Math.abs(targetLegion.location.x - legion.location.x) + Math.abs(targetLegion.location.y - legion.location.y);
          const distLabel = distance === 0 ? 'here' : `${distance} away`;
          const hasRoom = targetLegion.soldiers.length < 8;
          const leader = targetLegion.soldiers.find(s => s.isLeader);
          const shortName = leader ? leader.name : (targetLegion.soldiers[0]?.name || 'Empty');
          const fullName = getLegionName(targetLegion);
          return `<button class="legion-btn transfer-legion-btn" data-target-legion-id="${targetLegion.id}" style="background: #2a4a5a;" ${!hasRoom ? 'disabled' : ''} title="${fullName} (${targetLegion.soldiers.length}/8, ${distLabel})">→${shortName}</button>`;
        }).join('') : '';

        const hasTransferTargets = hasNearbyCities || hasNearbyLegions;
        const nearbyInfo = [];
        if (hasNearbyCities) nearbyInfo.push(`${nearbyCities.length} city${nearbyCities.length > 1 ? 'ies' : ''}`);
        if (hasNearbyLegions) nearbyInfo.push(`${nearbyLegions.length} legion${nearbyLegions.length > 1 ? 's' : ''}`);

        actionsHtml = `
          <div class="edit-instructions">
            Click a soldier to select, then click another position to move/swap
            ${hasTransferTargets ? `<br><span style="color: #8cf">${nearbyInfo.join(' and ')} nearby: Select a soldier to transfer</span>` : ''}
          </div>
          <div class="legion-actions">
            ${transferButtons}
            ${unassignButtons}
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

    const legionDisplayName = getLegionName(legion);

    this.legionPanel.innerHTML = `
      <div class="legion-header">
        <h3 style="color: ${ownerColor}; border: none; margin: 0; padding: 0;">${legionDisplayName}</h3>
        <div style="font-size: 10px; color: #888;">${isPlayer ? 'Your Legion' : `${legion.owner} Legion`}</div>
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

    // Unassign buttons - move selected soldier to a nearby city's roster
    const unassignBtns = this.legionPanel.querySelectorAll('.unassign-city-btn');
    unassignBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const cityId = btn.getAttribute('data-city-id');
        if (cityId && this.selectedSoldierId) {
          this.state = processAction(this.state, {
            type: 'unassign_soldier',
            legionId: legion.id,
            soldierId: this.selectedSoldierId,
            cityId,
          });
          this.selectedSoldierId = null;
          this.updateUI();
        }
      });
    });

    // Transfer buttons - move selected soldier to a nearby legion
    const transferBtns = this.legionPanel.querySelectorAll('.transfer-legion-btn');
    transferBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const targetLegionId = btn.getAttribute('data-target-legion-id');
        if (targetLegionId && this.selectedSoldierId) {
          this.state = processAction(this.state, {
            type: 'transfer_soldier',
            fromLegionId: legion.id,
            soldierId: this.selectedSoldierId,
            toLegionId: targetLegionId,
          });
          this.selectedSoldierId = null;
          this.updateUI();
        }
      });
    });
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

    // Get all player legions within range of the city for soldier recruitment
    const nearbyLegions = getLegionsInRangeOfCity(this.state, city, 'player');
    const hasNearbyLegions = nearbyLegions.length > 0;

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
            ${city.buildings.map(b => {
              const building = BUILDING_TYPES[b];
              const effects = this.formatBuildingEffects(building);
              return `<div class="existing-building-item">
                <span class="existing-building">${building.name}</span>
                ${effects ? `<span class="building-effects">${effects}</span>` : ''}
              </div>`;
            }).join('')}
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
              const effects = this.formatBuildingEffects(b);
              return `
                <div class="build-option">
                  <div class="build-option-info">
                    <div class="build-option-name">${b.name}</div>
                    <div class="build-option-cost">${b.cost} gold | ${b.buildTurns} turns</div>
                    ${effects ? `<div class="build-option-effects">${effects}</div>` : ''}
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

    // Determine which soldiers can be recruited based on buildings
    const recruitableSoldiers: SoldierTypeId[] = ['fighter']; // Always available
    for (const buildingId of city.buildings) {
      const building = BUILDING_TYPES[buildingId];
      for (const effect of building.effects) {
        if (effect.type === 'unlock_soldier' && !recruitableSoldiers.includes(effect.soldier)) {
          recruitableSoldiers.push(effect.soldier);
        }
      }
    }

    // Available soldiers section - can train to nearby legions or roster
    let soldiersHtml = '';
    if (city.occupationTurns === 0) {
      // Helper to check if a legion can receive more soldiers
      const getLegionCapacity = (legion: typeof nearbyLegions[0]) => {
        const queuedForLegion = city.buildQueue.filter(
          item => item.itemType === 'soldier' && item.targetLegionId === legion.id
        ).length;
        return 8 - legion.soldiers.length - queuedForLegion;
      };

      // Generate legion buttons for each soldier type
      const getLegionButtons = (sid: SoldierTypeId, canAfford: boolean) => {
        if (!hasNearbyLegions) return '';
        return nearbyLegions.map(legion => {
          const capacity = getLegionCapacity(legion);
          const legionFull = capacity <= 0;
          const distance = Math.abs(legion.location.x - city.coord.x) + Math.abs(legion.location.y - city.coord.y);
          const distLabel = distance === 0 ? 'here' : `${distance} away`;
          const fullName = getLegionName(legion);
          // Get leader's first name for short label
          const leader = legion.soldiers.find(s => s.isLeader);
          const shortName = leader ? leader.name : (legion.soldiers[0]?.name || 'Empty');
          return `
            <button class="build-option-btn legion-target-btn"
                    data-build-type="soldier"
                    data-build-id="${sid}"
                    data-target="legion"
                    data-legion-id="${legion.id}"
                    ${!canAfford || legionFull ? 'disabled' : ''}
                    title="${fullName} (${legion.soldiers.length}/8, ${distLabel})">
              →${shortName}
            </button>
          `;
        }).join('');
      };

      soldiersHtml = `
        <div class="city-section">
          <div class="city-section-title">Train Soldiers ${hasNearbyLegions ? `(${nearbyLegions.length} legion${nearbyLegions.length > 1 ? 's' : ''} nearby)` : ''}</div>
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
                  <div class="build-option-btns">
                    ${getLegionButtons(sid, canAfford)}
                    <button class="build-option-btn" data-build-type="soldier" data-build-id="${sid}" data-target="roster" ${!canAfford ? 'disabled' : ''} title="Train to City Roster">
                      →Roster
                    </button>
                  </div>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    }

    // Roster section - unassigned soldiers at this city
    let rosterHtml = '';
    if (city.roster.length > 0 || city.buildQueue.some(item => item.itemType === 'soldier' && !item.targetLegionId)) {
      const queuedForRoster = city.buildQueue.filter(item => item.itemType === 'soldier' && !item.targetLegionId).length;

      // Generate assign buttons for each nearby legion + New Legion option
      const getAssignButtons = (soldierId: string) => {
        let buttons = '';

        // Add buttons for existing nearby legions
        if (hasNearbyLegions) {
          buttons += nearbyLegions.map(legion => {
            const canAssign = legion.soldiers.length < 8;
            const distance = Math.abs(legion.location.x - city.coord.x) + Math.abs(legion.location.y - city.coord.y);
            const distLabel = distance === 0 ? 'here' : `${distance} away`;
            const fullName = getLegionName(legion);
            const leader = legion.soldiers.find(s => s.isLeader);
            const shortName = leader ? leader.name : (legion.soldiers[0]?.name || 'Empty');
            return `
              <button class="roster-assign-btn"
                      data-soldier-id="${soldierId}"
                      data-legion-id="${legion.id}"
                      ${!canAssign ? 'disabled' : ''}
                      title="${fullName} (${legion.soldiers.length}/8, ${distLabel})">
                →${shortName}
              </button>
            `;
          }).join('');
        }

        // Add "New Legion" button
        buttons += `
          <button class="roster-assign-btn new-legion-btn"
                  data-soldier-id="${soldierId}"
                  data-action="new-legion"
                  title="Create a new legion with this soldier as leader">
            +New Legion
          </button>
        `;

        return buttons;
      };

      rosterHtml = `
        <div class="city-section">
          <div class="city-section-title">City Roster (${city.roster.length}${queuedForRoster > 0 ? ` +${queuedForRoster} training` : ''})</div>
          <div class="roster-list">
            ${city.roster.length === 0 ? '<div class="roster-empty">No soldiers in roster</div>' : ''}
            ${city.roster.map(s => {
              const soldierType = SOLDIER_TYPES[s.type];
              const hpPercent = (s.hp / s.maxHp) * 100;
              const hpClass = hpPercent > 50 ? 'high' : hpPercent > 25 ? 'medium' : 'low';
              return `
                <div class="roster-soldier">
                  <div class="roster-soldier-info">
                    <div class="roster-soldier-icon ${s.type}">${s.type.slice(0, 3).toUpperCase()}</div>
                    <div class="roster-soldier-details">
                      <div class="roster-soldier-name">${s.name} <span style="color: #888; font-size: 9px">(${soldierType.name})</span></div>
                      <div class="roster-soldier-hp">
                        <div class="roster-hp-bar">
                          <div class="roster-hp-fill ${hpClass}" style="width: ${hpPercent}%"></div>
                        </div>
                        <span>${s.hp}/${s.maxHp}</span>
                      </div>
                    </div>
                  </div>
                  <div class="roster-assign-btns">
                    ${getAssignButtons(s.id)}
                  </div>
                </div>
              `;
            }).join('')}
          </div>
        </div>
      `;
    }

    const capitalLabel = city.isCapital ? ' (Capital)' : '';

    // Calculate city bonuses for display
    const cityIncome = getCityIncome(city);
    const growthInfo = getCityGrowthInfo(city);
    const defenseBonus = getCityDefenseBonus(city);

    // Build income display
    let incomeHtml = `<span style="color: #fc0">+${cityIncome.totalGold} gold/turn</span>`;
    if (cityIncome.totalMana > 0) {
      incomeHtml += ` <span style="color: #a8f">+${cityIncome.totalMana} mana/turn</span>`;
    }

    // Build growth display
    const growthPercent = (growthInfo.currentProgress / growthInfo.progressNeeded) * 100;
    const growthHtml = `
      <div class="growth-display">
        <span>Growth: ${growthInfo.turnsUntilGrowth} turn${growthInfo.turnsUntilGrowth !== 1 ? 's' : ''} to next pop</span>
        ${growthInfo.hasGranary ? '<span class="growth-bonus">(+Granary)</span>' : ''}
        <div class="growth-bar">
          <div class="growth-fill" style="width: ${growthPercent}%"></div>
        </div>
      </div>
    `;

    // Build defense display
    const defenseHtml = defenseBonus > 0
      ? `<span class="defense-indicator">Defense: +${defenseBonus}% (Walls)</span>`
      : '';

    this.cityPanel.innerHTML = `
      <h3>${city.name}</h3>
      <div style="font-size: 10px; color: #4af; margin-top: -8px; margin-bottom: 8px;">Your city${capitalLabel}</div>
      <div class="city-stats">
        <span>Population: ${city.population}</span>
        <span>Building Slots: ${usedSlots}/${slots}</span>
        ${incomeHtml}
        ${defenseHtml}
        ${city.occupationTurns > 0 ? `<span style="color: #f88">Occupied (${city.occupationTurns} turns)</span>` : ''}
        ${hasNearbyLegions ? `<span style="color: #8cf">${nearbyLegions.length} Legion${nearbyLegions.length > 1 ? 's' : ''} in Range</span>` : ''}
      </div>
      ${growthHtml}
      ${queueHtml}
      ${existingBuildingsHtml}
      ${buildingsHtml}
      ${soldiersHtml}
      ${rosterHtml}
    `;

    this.cityPanel.style.display = 'block';

    // Set up button listeners
    this.setupCityPanelListeners(city.id);
  }

  private setupCityPanelListeners(cityId: string): void {
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
        const target = btn.getAttribute('data-target');
        const legionId = btn.getAttribute('data-legion-id');
        if (!buildType || !buildId) return;

        if (buildType === 'building') {
          this.state = processAction(this.state, {
            type: 'queue_building',
            cityId,
            buildingId: buildId as BuildingId,
          });
        } else if (buildType === 'soldier') {
          // Train to specified legion or roster based on target
          const targetLegionId = target === 'legion' && legionId ? legionId : undefined;
          this.state = processAction(this.state, {
            type: 'queue_soldier',
            cityId,
            soldierType: buildId as SoldierTypeId,
            targetLegionId,
          });
        }
        this.updateUI();
      });
    });

    // Assign roster soldiers to legion or create new legion
    const assignBtns = this.cityPanel.querySelectorAll('.roster-assign-btn');
    assignBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const soldierId = btn.getAttribute('data-soldier-id');
        const action = btn.getAttribute('data-action');
        const legionId = btn.getAttribute('data-legion-id');

        if (!soldierId) return;

        if (action === 'new-legion') {
          // Create a new legion with this soldier as leader
          this.state = processAction(this.state, {
            type: 'create_legion_from_roster',
            cityId,
            soldierId,
          });
          this.updateUI();
        } else if (legionId) {
          // Assign to existing legion
          this.state = processAction(this.state, {
            type: 'assign_soldier',
            cityId,
            soldierId,
            legionId,
          });
          this.updateUI();
        }
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
    const leaderBadge = soldier.isLeader ? ' <span style="color: #fc0;">★ Leader</span>' : '';

    // Get attacks per row
    const attacksInRow = soldierType.attackCount[soldier.position.row];
    const targetPref = soldierType.attacksTarget === 'front' ? 'Front row first' : 'Back row first';

    this.unitTooltip.innerHTML = `
      <h4 style="color: ${this.getSoldierColor(soldier.type)}">${soldier.name}${leaderBadge}</h4>
      <div style="font-size: 11px; color: #888; margin-top: -4px; margin-bottom: 8px;">${soldierType.name}</div>

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

  private formatBuildingEffects(building: typeof BUILDING_TYPES[keyof typeof BUILDING_TYPES]): string {
    if (building.effects.length === 0) return '';

    const effectStrings = building.effects.map(effect => {
      switch (effect.type) {
        case 'unlock_soldier':
          return `Unlocks ${SOLDIER_TYPES[effect.soldier].name}`;
        case 'gold_per_turn':
          return `+${effect.amount} gold/turn`;
        case 'mana_per_turn':
          return `+${effect.amount} mana/turn`;
        case 'defense_bonus':
          return `+${effect.amount}% defense`;
        case 'growth_bonus':
          return `+${effect.amount} growth/turn`;
        default:
          return '';
      }
    }).filter(s => s);

    return effectStrings.join(', ');
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

  // ============ Collegia Panel ============

  private updateResearchIndicator(): void {
    const researchStatus = document.getElementById('research-status');
    const researchIndicator = document.getElementById('research-indicator');
    if (!researchStatus || !researchIndicator) return;

    const { collegia } = this.state;

    if (collegia.currentResearch) {
      const tech = TECHNOLOGIES[collegia.currentResearch.technologyId];
      researchStatus.textContent = `${tech.name} (${collegia.currentResearch.turnsRemaining})`;
      researchIndicator.classList.remove('needs-selection');
    } else {
      researchStatus.textContent = 'Select Research';
      researchIndicator.classList.add('needs-selection');
    }
  }

  private toggleCollegiaPanel(): void {
    this.collegiaVisible = !this.collegiaVisible;
    if (this.collegiaVisible) {
      this.updateCollegiaPanel();
      this.collegiaPanel.style.display = 'block';
    } else {
      this.collegiaPanel.style.display = 'none';
    }
  }

  private formatTechEffects(tech: typeof TECHNOLOGIES[keyof typeof TECHNOLOGIES]): string {
    return tech.effects.map(effect => {
      switch (effect.type) {
        case 'unlock_building':
          return `Unlocks ${BUILDING_TYPES[effect.building].name}`;
        case 'soldier_attack_bonus':
          return `${SOLDIER_TYPES[effect.soldier].name} +${effect.amount} ATK`;
        case 'soldier_defense_bonus':
          return `${SOLDIER_TYPES[effect.soldier].name} +${effect.amount} DEF`;
        case 'soldier_hp_bonus':
          return `${SOLDIER_TYPES[effect.soldier].name} +${effect.amount} HP`;
        case 'building_gold_bonus':
          return `${BUILDING_TYPES[effect.building].name} +${effect.amount} gold`;
        case 'building_mana_bonus':
          return `${BUILDING_TYPES[effect.building].name} +${effect.amount} mana`;
        case 'global_gold_bonus':
          return `+${effect.amount} gold/turn`;
        case 'global_mana_bonus':
          return `+${effect.amount} mana/turn`;
        case 'global_growth_bonus':
          return `+${effect.amount} growth`;
        case 'global_defense_bonus':
          return `+${effect.amount}% defense`;
        case 'legion_movement_bonus':
          return `+${effect.amount} movement`;
        default:
          return '';
      }
    }).filter(s => s).join(', ');
  }

  private formatTierName(tier: string): string {
    return tier.replace(/_/g, ' ').replace(/\b\w/g, c => c.toUpperCase());
  }

  private updateCollegiaPanel(): void {
    const { collegia } = this.state;
    const player = this.state.factions.get('player');
    const canReroll = collegia.rerollAvailable && !collegia.currentResearch && player && player.gold >= 100;

    // Current research section
    let currentResearchHtml = '';
    if (collegia.currentResearch) {
      const tech = TECHNOLOGIES[collegia.currentResearch.technologyId];
      const totalTurns = tech.researchTurns;
      const elapsed = totalTurns - collegia.currentResearch.turnsRemaining;
      const progressPercent = (elapsed / totalTurns) * 100;

      currentResearchHtml = `
        <div class="collegia-current-research">
          <h4>Currently Researching: ${tech.name}</h4>
          <div style="font-size: 11px; color: #aaa;">${tech.description}</div>
          <div style="font-size: 10px; color: #8cf; margin-top: 4px;">${this.formatTechEffects(tech)}</div>
          <div style="font-size: 12px; margin-top: 8px;">
            <span style="color: #ffa500">${collegia.currentResearch.turnsRemaining}</span>
            <span style="color: #888"> turns remaining</span>
          </div>
          <div class="research-progress-bar">
            <div class="research-progress-fill" style="width: ${progressPercent}%"></div>
          </div>
        </div>
      `;
    }

    // Offerings section (only show if not researching)
    let offeringsHtml = '';
    if (!collegia.currentResearch) {
      offeringsHtml = `
        <div class="city-section-title" style="color: #d2691e; margin-bottom: 8px;">Choose Your Next Study</div>
        <div class="collegia-offerings">
          ${collegia.availableOfferings.map(techId => {
            const tech = TECHNOLOGIES[techId];
            return `
              <div class="collegia-offering">
                <div class="offering-info">
                  <div class="offering-name">${tech.name}</div>
                  <div class="offering-description">${tech.description}</div>
                  <div class="offering-meta">
                    <span class="offering-category ${tech.category}">${tech.category}</span>
                    <span class="offering-tier ${tech.tier}">${this.formatTierName(tech.tier)}</span>
                  </div>
                  <div class="offering-effects">${this.formatTechEffects(tech)}</div>
                </div>
                <div class="offering-select">
                  <div class="offering-turns">${tech.researchTurns}</div>
                  <div class="offering-turns-label">turns</div>
                  <button class="offering-select-btn" data-tech-id="${techId}">Study</button>
                </div>
              </div>
            `;
          }).join('')}
        </div>
      `;
    }

    // Owned technologies section
    let ownedHtml = '';
    if (collegia.ownedTechnologies.length > 0) {
      ownedHtml = `
        <div class="collegia-owned">
          <div class="collegia-owned-title">Learned (${collegia.ownedTechnologies.length})</div>
          <div class="owned-tech-list">
            ${collegia.ownedTechnologies.map(techId => {
              const tech = TECHNOLOGIES[techId];
              return `<span class="owned-tech" title="${tech.description}">${tech.name}</span>`;
            }).join('')}
          </div>
        </div>
      `;
    }

    this.collegiaPanel.innerHTML = `
      <h3>The Wandering Collegia</h3>
      ${currentResearchHtml}
      ${offeringsHtml}
      <div class="collegia-actions">
        <button class="collegia-btn reroll" id="collegia-reroll" ${!canReroll ? 'disabled' : ''}>
          Reroll (100 gold)${!collegia.rerollAvailable ? ' - Used' : ''}
        </button>
        <button class="collegia-btn close" id="collegia-close">Close</button>
      </div>
      ${ownedHtml}
    `;

    // Add event listeners
    const closeBtn = document.getElementById('collegia-close');
    closeBtn?.addEventListener('click', () => this.toggleCollegiaPanel());

    const rerollBtn = document.getElementById('collegia-reroll');
    rerollBtn?.addEventListener('click', () => {
      this.state = processAction(this.state, { type: 'reroll_collegia' });
      this.updateUI();
    });

    const selectBtns = this.collegiaPanel.querySelectorAll('.offering-select-btn');
    selectBtns.forEach(btn => {
      btn.addEventListener('click', () => {
        const techId = btn.getAttribute('data-tech-id');
        if (techId) {
          this.state = processAction(this.state, { type: 'select_research', technologyId: techId });
          this.updateUI();
        }
      });
    });
  }

  // ============ Knowledge Panel ============

  private updateKnowledgeCount(): void {
    const knowledgeCount = document.getElementById('knowledge-count');
    if (!knowledgeCount) return;

    const count = this.state.collegia.ownedTechnologies.length;
    knowledgeCount.textContent = `${count} Learned`;
  }

  private toggleKnowledgePanel(): void {
    this.knowledgeVisible = !this.knowledgeVisible;
    if (this.knowledgeVisible) {
      this.updateKnowledgePanel();
      this.knowledgePanel.style.display = 'block';
    } else {
      this.knowledgePanel.style.display = 'none';
    }
  }

  private updateKnowledgePanel(): void {
    const { collegia } = this.state;

    if (collegia.ownedTechnologies.length === 0) {
      this.knowledgePanel.innerHTML = `
        <h3>Learned Technologies</h3>
        <div class="knowledge-empty">
          No technologies learned yet.<br>
          Visit the Collegia to begin your studies.
        </div>
        <button class="knowledge-close" id="knowledge-close">Close</button>
      `;
    } else {
      // Group technologies by category
      const byCategory: Record<string, typeof TECHNOLOGIES[keyof typeof TECHNOLOGIES][]> = {
        martial: [],
        industrial: [],
        arcane: [],
        social: [],
      };

      for (const techId of collegia.ownedTechnologies) {
        const tech = TECHNOLOGIES[techId];
        if (tech) {
          byCategory[tech.category].push(tech);
        }
      }

      // Build HTML for each category that has technologies
      const categoriesHtml = Object.entries(byCategory)
        .filter(([_, techs]) => techs.length > 0)
        .map(([category, techs]) => `
          <div class="knowledge-category">
            <div class="knowledge-category-header ${category}">${category.charAt(0).toUpperCase() + category.slice(1)}</div>
            <div class="knowledge-tech-list">
              ${techs.map(tech => `
                <div class="knowledge-tech">
                  <div class="knowledge-tech-header">
                    <span class="knowledge-tech-name">${tech.name}</span>
                    <span class="knowledge-tech-tier ${tech.tier}">${this.formatTierName(tech.tier)}</span>
                  </div>
                  <div class="knowledge-tech-description">${tech.description}</div>
                  <div class="knowledge-tech-effects">${this.formatTechEffects(tech)}</div>
                </div>
              `).join('')}
            </div>
          </div>
        `).join('');

      this.knowledgePanel.innerHTML = `
        <h3>Learned Technologies (${collegia.ownedTechnologies.length})</h3>
        <div class="knowledge-categories">
          ${categoriesHtml}
        </div>
        <button class="knowledge-close" id="knowledge-close">Close</button>
      `;
    }

    // Add close button listener
    const closeBtn = document.getElementById('knowledge-close');
    closeBtn?.addEventListener('click', () => this.toggleKnowledgePanel());
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
