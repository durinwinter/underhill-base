/**
 * Sabatier Reactor System
 * Procedural Three.js Scene for Underhill Base ISRU
 *
 * The Sabatier reaction: CO₂ + 4H₂ → CH₄ + 2H₂O
 *
 * Systems:
 * - Main Reactor Vessel: High-temperature catalytic bed (can react at 200-500°C)
 * - Hydrogen Supply: Storage and metering system
 * - CO₂ Input: From atmosphere scrubber or tank
 * - Product Recovery: Methane collection and water separation
 * - Heat Management: Thermal control and recovery
 * - Instrumentation: Temperature, pressure, flow monitoring
 */

import * as THREE from "three";
import { OrbitControls } from "https://unpkg.com/three@0.161.0/examples/jsm/controls/OrbitControls.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const SABATIER_COLORS = {
  MAIN_VESSEL: 0x4a3a5a,
  CATALYST_BED: 0x8a6a4a,
  HEATER_COIL: 0xaa4a2a,
  INSULATION: 0x6a6a5a,
  PIPE_H2: 0x4a6aaa,    // Hydrogen (blue)
  PIPE_CO2: 0x9a4a4a,   // CO2 (red)
  PIPE_CH4: 0x4aaa4a,   // Methane (green)
  PIPE_H2O: 0x6a9aff,   // Water (light blue)
  VALVE: 0xcc9944,
  PUMP: 0x5a7a6a,
  TANK: 0x5a5a7a,
  RADIATOR: 0x5a7a9a,
  INSTRUMENTATION: 0xcccc00,
};

const REACTOR_SCALE = 0.75;

// ---------------------------------------------------------------------------
// SabatierScene
// ---------------------------------------------------------------------------
export class SabatierScene {
  constructor(container) {
    this._container = container;
    this._lastSnapshot = null;
    this._reactorTemperature = 0.3; // 0.0-1.0 (cold to hot)
    this._reactionProgress = 0;

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0f0f1a);

    this._initRenderer(container);
    this._initCameraAndControls(container);
    this._initLighting();
    this._initEnvironment();
    this._buildSabatierSystem();
    this._startAnimationLoop();
  }

  _initRenderer(container) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.setSize(container.clientWidth, container.clientHeight);
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowShadowMap;
    container.appendChild(this.renderer.domElement);

    window.addEventListener("resize", () => this._onWindowResize(container), false);
  }

  _onWindowResize(container) {
    const w = container.clientWidth;
    const h = container.clientHeight;
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  _initCameraAndControls(container) {
    const w = container.clientWidth;
    const h = container.clientHeight;
    this.camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 200);
    this.camera.position.set(7, 4, 7);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0, 2, 0);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 4;
    this.controls.maxDistance = 25;
    this.controls.autoRotate = true;
    this.controls.autoRotateSpeed = 0.6;

    let idleTimer = null;
    this.controls.addEventListener("start", () => {
      this.controls.autoRotate = false;
      clearTimeout(idleTimer);
    });
    this.controls.addEventListener("end", () => {
      idleTimer = setTimeout(() => { this.controls.autoRotate = true; }, 8000);
    });
  }

  _initLighting() {
    // High-intensity work lights
    const light1 = new THREE.DirectionalLight(0xffffdd, 1.2);
    light1.position.set(12, 10, 8);
    light1.castShadow = true;
    light1.shadow.mapSize.width = 2048;
    light1.shadow.mapSize.height = 2048;
    light1.shadow.camera.near = 0.5;
    light1.shadow.camera.far = 50;
    light1.shadow.camera.left = -20;
    light1.shadow.camera.right = 20;
    light1.shadow.camera.top = 20;
    light1.shadow.camera.bottom = -20;
    this.scene.add(light1);

    const light2 = new THREE.DirectionalLight(0xccbbff, 0.7);
    light2.position.set(-10, 7, -6);
    this.scene.add(light2);

    // Ambient
    this.scene.add(new THREE.AmbientLight(0x7a8a9a, 0.5));
  }

  _initEnvironment() {
    // Lab floor
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(35, 35),
      new THREE.MeshLambertMaterial({ color: 0x2a2a3a })
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = -0.05;
    floor.receiveShadow = true;
    this.scene.add(floor);

    // Work table
    const tableGeo = new THREE.BoxGeometry(15, 0.4, 8);
    const tableMat = new THREE.MeshLambertMaterial({ color: 0x4a4a5a });
    const table = new THREE.Mesh(tableGeo, tableMat);
    table.position.y = 0;
    table.receiveShadow = true;
    this.scene.add(table);

    // Back panel
    const panel = new THREE.Mesh(
      new THREE.BoxGeometry(15, 10, 0.3),
      new THREE.MeshLambertMaterial({ color: 0x3a3a4a })
    );
    panel.position.z = -4.5;
    panel.position.y = 4;
    this.scene.add(panel);
  }

  _buildSabatierSystem() {
    const root = new THREE.Group();
    root.scale.setScalar(REACTOR_SCALE);
    this.scene.add(root);

    // Plant layout (left to right): H2 Tank → Reactor Core → Heat Recovery → Product Sep

    // ========== Hydrogen Supply System ==========
    this._buildH2SupplySystem(root, -4, 0.5, 0);

    // ========== Main Sabatier Reactor Vessel ==========
    this._buildMainReactorVessel(root, 0, 0.5, 0);

    // ========== Heat Recovery System ==========
    this._buildHeatRecovery(root, 3.5, 0.5, 0);

    // ========== Product Separator & CH4 Storage ==========
    this._buildProductSeparation(root, -2, 2.8, 1.5);

    // ========== Control Panel ==========
    this._buildControlPanel(root, 4, 2, 0);

    // ========== Process Piping Interconnects ==========
    this._buildProcessPiping(root);
  }

  // =========================================================================
  // Hydrogen Supply System
  // =========================================================================
  _buildH2SupplySystem(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // H2 Storage Tank (high-pressure)
    const tankGeo = new THREE.CylinderGeometry(0.5, 0.5, 1.5, 12);
    const tankMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.TANK,
      roughness: 0.5,
      metalness: 0.7,
    });
    const tank = new THREE.Mesh(tankGeo, tankMat);
    tank.castShadow = true;
    tank.receiveShadow = true;
    group.add(tank);

    // Tank cap
    const capGeo = new THREE.SphereGeometry(0.5, 12, 8, 0, Math.PI * 2, 0, Math.PI / 2);
    const cap = new THREE.Mesh(capGeo, tankMat);
    cap.position.y = 0.75;
    cap.castShadow = true;
    group.add(cap);

    // Pressure gauge
    const gaugeGeo = new THREE.TorusGeometry(0.2, 0.04, 8, 16);
    const gaugeMat = new THREE.MeshStandardMaterial({
      color: 0xcccccc,
      metalness: 0.95,
    });
    const gauge = new THREE.Mesh(gaugeGeo, gaugeMat);
    gauge.position.set(0.65, 0.3, 0);
    gauge.rotation.x = Math.PI / 2;
    group.add(gauge);

    // Pressure relief valve (safety device)
    this._buildPressureValve(group, 0, 1.0, 0);

    // Outlet port with isolation valve
    const outletValveGeo = new THREE.CylinderGeometry(0.12, 0.1, 0.25, 6);
    const outletValveMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.VALVE,
      metalness: 0.8,
    });
    const outletValve = new THREE.Mesh(outletValveGeo, outletValveMat);
    outletValve.position.set(0.6, 0, 0);
    outletValve.rotation.z = Math.PI / 4;
    outletValve.castShadow = true;
    group.add(outletValve);

    // Mass flow controller
    const flowControlGeo = new THREE.BoxGeometry(0.3, 0.3, 0.5);
    const flowControlMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.INSTRUMENTATION,
      roughness: 0.4,
    });
    const flowControl = new THREE.Mesh(flowControlGeo, flowControlMat);
    flowControl.position.set(0.6, 0.5, 0);
    flowControl.castShadow = true;
    group.add(flowControl);

    // LED status
    const statusGeo = new THREE.SphereGeometry(0.1, 8, 8);
    const statusMat = new THREE.MeshStandardMaterial({
      color: 0xaa7744,
      emissive: 0x664422,
    });
    const status = new THREE.Mesh(statusGeo, statusMat);
    status.position.set(-0.5, 0.8, 0.3);
    group.add(status);

    this._addLabel(group, 0, -1.0, 0, "H₂ Supply", 0x4a6aff);
  }

  // =========================================================================
  // Main Sabatier Reactor Vessel
  // =========================================================================
  _buildMainReactorVessel(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Outer pressure vessel (stainless steel)
    const vesselGeo = new THREE.CylinderGeometry(0.7, 0.7, 2.0, 16);
    const vesselMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.MAIN_VESSEL,
      roughness: 0.4,
      metalness: 0.85,
    });
    const vessel = new THREE.Mesh(vesselGeo, vesselMat);
    vessel.castShadow = true;
    vessel.receiveShadow = true;
    group.add(vessel);

    // Hemispherical dished end (bottom)
    const bottomEndGeo = new THREE.SphereGeometry(0.7, 16, 8, 0, Math.PI * 2, Math.PI / 2, Math.PI / 2);
    const bottomEnd = new THREE.Mesh(bottomEndGeo, vesselMat);
    bottomEnd.position.y = -1.0;
    bottomEnd.castShadow = true;
    group.add(bottomEnd);

    // Hemispherical dished end (top)
    const topEndGeo = new THREE.SphereGeometry(0.7, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2);
    const topEnd = new THREE.Mesh(topEndGeo, vesselMat);
    topEnd.position.y = 1.0;
    topEnd.castShadow = true;
    group.add(topEnd);

    // Thermal insulation wrap (outer layer)
    const insulationGeo = new THREE.CylinderGeometry(0.75, 0.75, 2.05, 16);
    const insulationMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.INSULATION,
      roughness: 0.9,
      metalness: 0.1,
    });
    const insulation = new THREE.Mesh(insulationGeo, insulationMat);
    insulation.position.z = -0.05;
    group.add(insulation);

    // Catalyst bed visualization (internal bright core)
    const catalystGeo = new THREE.CylinderGeometry(0.4, 0.4, 1.6, 8);
    const catalystMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.CATALYST_BED,
      emissive: new THREE.Color(SABATIER_COLORS.CATALYST_BED).multiplyScalar(0.3),
      metalness: 0.6,
    });
    const catalyst = new THREE.Mesh(catalystGeo, catalystMat);
    group.add(catalyst);

    // Heating coil around vessel
    for (let i = -0.8; i <= 0.8; i += 0.35) {
      const coilGeo = new THREE.TorusGeometry(0.9, 0.06, 6, 12);
      const coilMat = new THREE.MeshStandardMaterial({
        color: SABATIER_COLORS.HEATER_COIL,
        emissive: this._interpolateColor(
          new THREE.Color(SABATIER_COLORS.HEATER_COIL),
          new THREE.Color(0xff4400),
          this._reactorTemperature
        ),
        metalness: 0.8,
      });
      const coil = new THREE.Mesh(coilGeo, coilMat);
      coil.position.y = i;
      coil.rotation.x = Math.PI / 2;
      group.add(coil);
    }

    // ========== Inlet/Outlet Ports ==========

    // CO2 inlet (top-left)
    const co2InletGeo = new THREE.CylinderGeometry(0.15, 0.12, 0.35, 8);
    const co2InletMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.VALVE,
      metalness: 0.75,
    });
    const co2Inlet = new THREE.Mesh(co2InletGeo, co2InletMat);
    co2Inlet.position.set(-0.9, 0.3, 0);
    co2Inlet.rotation.z = Math.PI / 8;
    co2Inlet.castShadow = true;
    group.add(co2Inlet);
    this._addSmallLabel(group, -1.3, 0.6, 0, "CO₂ In");

    // H2 inlet (top-right)
    const h2InletGeo = new THREE.CylinderGeometry(0.12, 0.1, 0.3, 8);
    const h2Inlet = new THREE.Mesh(h2InletGeo, co2InletMat);
    h2Inlet.position.set(0.9, 0.3, 0);
    h2Inlet.rotation.z = -Math.PI / 8;
    h2Inlet.castShadow = true;
    group.add(h2Inlet);
    this._addSmallLabel(group, 1.3, 0.6, 0, "H₂ In");

    // Product outlet (bottom)
    const productOutletGeo = new THREE.CylinderGeometry(0.15, 0.12, 0.35, 8);
    const productOutlet = new THREE.Mesh(productOutletGeo, co2InletMat);
    productOutlet.position.set(0, -1.2, 0);
    productOutlet.castShadow = true;
    group.add(productOutlet);
    this._addSmallLabel(group, 0, -1.5, 0.3, "(CH₄ + H₂O)");

    // ========== Temperature Sensor ==========
    const tempSensorGeo = new THREE.CylinderGeometry(0.08, 0.08, 0.5, 6);
    const tempSensorMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.INSTRUMENTATION,
      metalness: 0.9,
    });
    const tempSensor = new THREE.Mesh(tempSensorGeo, tempSensorMat);
    tempSensor.position.set(0.8, -0.3, 0.8);
    tempSensor.rotation.z = Math.PI / 6;
    group.add(tempSensor);

    // Temperature readout (glowing indicator)
    const tempReadoutGeo = new THREE.SphereGeometry(0.12, 8, 8);
    const tempColor = this._interpolateColor(
      new THREE.Color(0x0088ff),
      new THREE.Color(0xff2200),
      this._reactorTemperature
    );
    const tempReadoutMat = new THREE.MeshStandardMaterial({
      color: tempColor,
      emissive: tempColor.clone().multiplyScalar(0.6),
    });
    const tempReadout = new THREE.Mesh(tempReadoutGeo, tempReadoutMat);
    tempReadout.position.set(1.2, -0.3, 0.8);
    group.add(tempReadout);

    this._addLabel(group, 0, -1.8, 0, "Main Reactor", 0xffcc77);
  }

  // =========================================================================
  // Heat Recovery System
  // =========================================================================
  _buildHeatRecovery(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Product cooler (counterflow)
    const coolerGeo = new THREE.BoxGeometry(0.6, 1.2, 0.8);
    const coolerMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.RADIATOR,
      roughness: 0.5,
      metalness: 0.7,
    });
    const cooler = new THREE.Mesh(coolerGeo, coolerMat);
    cooler.castShadow = true;
    group.add(cooler);

    // Internal fins (visualization)
    for (let i = 0; i < 4; i++) {
      const finGeo = new THREE.BoxGeometry(0.55, 0.2, 0.08);
      const fin = new THREE.Mesh(finGeo, coolerMat);
      fin.position.y = -0.3 + (i * 0.3);
      fin.castShadow = true;
      group.add(fin);
    }

    // Hot side inlet
    const hotInletGeo = new THREE.CylinderGeometry(0.1, 0.1, 0.2, 6);
    const hotInletMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.PIPE_H2O,
      metalness: 0.8,
    });
    const hotInlet = new THREE.Mesh(hotInletGeo, hotInletMat);
    hotInlet.position.set(-0.35, 0.5, 0);
    hotInlet.rotation.z = Math.PI / 6;
    group.add(hotInlet);

    // Cold side outlet
    const coldOutletGeo = new THREE.CylinderGeometry(0.1, 0.1, 0.2, 6);
    const coldOutlet = new THREE.Mesh(coldOutletGeo, hotInletMat);
    coldOutlet.position.set(0.35, -0.5, 0);
    coldOutlet.rotation.z = -Math.PI / 6;
    group.add(coldOutlet);

    // External radiator fins
    for (let i = 0; i < 3; i++) {
      const radiatorFinGeo = new THREE.BoxGeometry(0.8, 0.08, 0.3);
      const radiatorFinMat = new THREE.MeshStandardMaterial({
        color: SABATIER_COLORS.RADIATOR,
        metalness: 0.9,
      });
      const radiatorFin = new THREE.Mesh(radiatorFinGeo, radiatorFinMat);
      radiatorFin.position.z = -0.5 + (i * 0.5);
      group.add(radiatorFin);
    }

    this._addLabel(group, 0, -1.0, 0, "Heat\nRecovery", 0x88ccff);
  }

  // =========================================================================
  // Product Separation & Methane Storage
  // =========================================================================
  _buildProductSeparation(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Water trap (knock-out vessel)
    const trapGeo = new THREE.CylinderGeometry(0.4, 0.4, 0.8, 12);
    const trapMat = new THREE.MeshStandardMaterial({
      color: 0x4a7a9a,
      roughness: 0.6,
      metalness: 0.6,
    });
    const trap = new THREE.Mesh(trapGeo, trapMat);
    trap.castShadow = true;
    group.add(trap);

    // Hemispherical bottom
    const trapBottomGeo = new THREE.SphereGeometry(0.4, 12, 8, 0, Math.PI * 2, Math.PI / 2, Math.PI / 2);
    const trapBottom = new THREE.Mesh(trapBottomGeo, trapMat);
    trapBottom.position.y = -0.4;
    trapBottom.castShadow = true;
    group.add(trapBottom);

    // Drain valve
    const drainGeo = new THREE.CylinderGeometry(0.08, 0.06, 0.15, 6);
    const drainMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.VALVE,
      metalness: 0.8,
    });
    const drain = new THREE.Mesh(drainGeo, drainMat);
    drain.position.set(-0.6, -0.6, 0);
    drain.rotation.z = Math.PI / 4;
    group.add(drain);

    // Water level indicator
    const levelGeo = new THREE.BoxGeometry(0.15, 0.4, 0.1);
    const levelMat = new THREE.MeshStandardMaterial({
      color: 0x6a9aff,
      transparent: true,
      opacity: 0.6,
    });
    const level = new THREE.Mesh(levelGeo, levelMat);
    level.position.set(0.65, -0.1, 0);
    group.add(level);

    // Methane storage cylinder (compact)
    const ch4TankGeo = new THREE.CylinderGeometry(0.35, 0.35, 1.2, 12);
    const ch4TankMat = new THREE.MeshStandardMaterial({
      color: 0x4a7a3a,
      roughness: 0.55,
      metalness: 0.65,
    });
    const ch4Tank = new THREE.Mesh(ch4TankGeo, ch4TankMat);
    ch4Tank.position.set(1.2, 0, 0);
    ch4Tank.castShadow = true;
    group.add(ch4Tank);

    // CH4 tank cap
    const ch4CapGeo = new THREE.SphereGeometry(0.35, 12, 8, 0, Math.PI * 2, 0, Math.PI / 2);
    const ch4Cap = new THREE.Mesh(ch4CapGeo, ch4TankMat);
    ch4Cap.position.set(1.2, 0.6, 0);
    ch4Cap.castShadow = true;
    group.add(ch4Cap);

    // Product flow line from separator to storage
    const flowLineGeo = new THREE.CylinderGeometry(0.08, 0.08, 0.4, 6);
    const flowLineMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.PIPE_CH4,
      metalness: 0.7,
    });
    const flowLine = new THREE.Mesh(flowLineGeo, flowLineMat);
    flowLine.position.set(0.6, 0, 0);
    flowLine.rotation.z = Math.PI / 2;
    group.add(flowLine);

    // Product purity indicator
    const purityGeo = new THREE.SphereGeometry(0.1, 8, 8);
    const purityMat = new THREE.MeshStandardMaterial({
      color: 0x22dd22,
      emissive: 0x11aa11,
    });
    const purity = new THREE.Mesh(purityGeo, purityMat);
    purity.position.set(1.2, 0.9, 0.4);
    group.add(purity);

    this._addLabel(group, 0.6, -0.9, 0, "Product\nSeparation", 0x88ff88);
  }

  // =========================================================================
  // Control Panel
  // =========================================================================
  _buildControlPanel(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Panel enclosure
    const panelGeo = new THREE.BoxGeometry(1.0, 1.2, 0.25);
    const panelMat = new THREE.MeshStandardMaterial({
      color: 0x3a3a4a,
      roughness: 0.3,
      metalness: 0.5,
    });
    const panel = new THREE.Mesh(panelGeo, panelMat);
    panel.castShadow = true;
    group.add(panel);

    // Buttons and displays
    const buttonPositions = [
      [-0.3, 0.3, "START"],
      [0.3, 0.3, "STOP"],
      [-0.3, 0, "PURGE"],
      [0.3, 0, "VENT"],
      [-0.3, -0.3, "ALARM"],
      [0.3, -0.3, "RESET"],
    ];

    buttonPositions.forEach(([px, py, label]) => {
      // Button body
      const btnGeo = new THREE.SphereGeometry(0.12, 8, 8);
      const btnMat = new THREE.MeshStandardMaterial({
        color: label.includes("START") ? 0x00cc00 : 0xaa3333,
        metalness: 0.8,
        emissive: label.includes("ALARM") ? 0xaa3333 : 0x000000,
      });
      const btn = new THREE.Mesh(btnGeo, btnMat);
      btn.position.set(px, py, 0.15);
      group.add(btn);
    });

    // Digital display screen
    const screenGeo = new THREE.BoxGeometry(0.7, 0.4, 0.05);
    const screenMat = new THREE.MeshStandardMaterial({
      color: 0x001a00,
      emissive: 0x003300,
      metalness: 0.3,
    });
    const screen = new THREE.Mesh(screenGeo, screenMat);
    screen.position.z = 0.15;
    screen.position.y = 0.45;
    group.add(screen);

    // Screen content (simplified glow)
    const scrGeo = new THREE.BoxGeometry(0.65, 0.35, 0.01);
    const scrMat = new THREE.MeshStandardMaterial({
      color: 0x00ff00,
      emissive: 0x00aa00,
      transparent: true,
      opacity: 0.3,
    });
    const scr = new THREE.Mesh(scrGeo, scrMat);
    scr.position.z = 0.16;
    scr.position.y = 0.45;
    group.add(scr);

    // Status LEDs (top)
    const statusLEDs = [
      [-0.3, 0.65, 0x22dd22],
      [0, 0.65, 0xffdd00],
      [0.3, 0.65, 0xaa3333],
    ];

    statusLEDs.forEach(([lx, ly, color]) => {
      const ledGeo = new THREE.SphereGeometry(0.08, 6, 6);
      const ledMat = new THREE.MeshStandardMaterial({
        color: color,
        emissive: color,
      });
      const led = new THREE.Mesh(ledGeo, ledMat);
      led.position.set(lx, ly, 0.15);
      group.add(led);
    });

    this._addLabel(group, 0, -0.75, 0, "Control\nPanel", 0xcccc00);
  }

  // =========================================================================
  // Process Piping Interconnects
  // =========================================================================
  _buildProcessPiping(parent) {
    // H2 line: supply to reactor inlet
    this._buildPipeLine(parent, [
      [-3.5, 0.5, 0],
      [-2, 0.5, 0],
      [-0.9, 0.3, 0],
    ], 0.06, SABATIER_COLORS.PIPE_H2);

    // CO2 line: inject from left
    this._buildPipeLine(parent, [
      [-3, 2, 2],
      [-1.5, 0.8, 0.5],
      [-0.9, 0.3, 0],
    ], 0.06, SABATIER_COLORS.PIPE_CO2);

    // Product line: reactor to separator
    this._buildPipeLine(parent, [
      [0, -1.2, 0],
      [0, -1.5, 0.3],
      [-2, 1.8, 1.5],
    ], 0.07, SABATIER_COLORS.PIPE_CH4);

    // Heat recovery loop
    this._buildPipeLine(parent, [
      [0, -0.8, 0],
      [1.5, -0.3, 0],
      [3.5, 0.5, 0],
    ], 0.06, SABATIER_COLORS.PIPE_H2O);
  }

  // =========================================================================
  // Helper Methods
  // =========================================================================

  _buildPressureValve(group, x, y, z) {
    const valveGeo = new THREE.CylinderGeometry(0.15, 0.1, 0.3, 6);
    const valveMat = new THREE.MeshStandardMaterial({
      color: SABATIER_COLORS.VALVE,
      roughness: 0.4,
      metalness: 0.8,
    });
    const valve = new THREE.Mesh(valveGeo, valveMat);
    valve.position.set(x, y, z);
    valve.rotation.z = Math.PI / 4;
    valve.castShadow = true;
    group.add(valve);
  }

  _buildPipeLine(parent, points, radius, color) {
    for (let i = 0; i < points.length - 1; i++) {
      const start = new THREE.Vector3(...points[i]);
      const end = new THREE.Vector3(...points[i + 1]);
      const mid = start.clone().add(end).multiplyScalar(0.5);
      const length = start.distanceTo(end);
      const direction = end.clone().sub(start).normalize();

      const pipeGeo = new THREE.CylinderGeometry(radius, radius, length, 6);
      const pipeMat = new THREE.MeshStandardMaterial({
        color: color,
        metalness: 0.7,
        roughness: 0.3,
      });
      const pipe = new THREE.Mesh(pipeGeo, pipeMat);
      pipe.position.copy(mid);
      pipe.lookAt(end);
      pipe.rotation.x += Math.PI / 2;
      pipe.castShadow = true;
      parent.add(pipe);

      // Add periodic joints/couplings
      if (i < points.length - 2) {
        const couplingGeo = new THREE.SphereGeometry(radius * 2, 6, 6);
        const couplingMat = new THREE.MeshStandardMaterial({
          color: 0xaa7744,
          metalness: 0.8,
        });
        const coupling = new THREE.Mesh(couplingGeo, couplingMat);
        coupling.position.copy(end);
        coupling.scale.z = 0.6;
        parent.add(coupling);
      }
    }
  }

  _addLabel(group, x, y, z, text, color) {
    const markerGeo = new THREE.SphereGeometry(0.15, 8, 8);
    const markerMat = new THREE.MeshStandardMaterial({
      color: color,
      emissive: color,
      metalness: 0.3,
    });
    const marker = new THREE.Mesh(markerGeo, markerMat);
    marker.position.set(x, y, z);
    group.add(marker);
  }

  _addSmallLabel(group, x, y, z, text) {
    const dotGeo = new THREE.SphereGeometry(0.06, 6, 6);
    const dotMat = new THREE.MeshStandardMaterial({
      color: 0xffbb99,
      metalness: 0.5,
    });
    const dot = new THREE.Mesh(dotGeo, dotMat);
    dot.position.set(x, y, z);
    group.add(dot);
  }

  _interpolateColor(colorA, colorB, t) {
    const result = new THREE.Color();
    result.r = colorA.r + (colorB.r - colorA.r) * t;
    result.g = colorA.g + (colorB.g - colorA.g) * t;
    result.b = colorA.b + (colorB.b - colorA.b) * t;
    return result;
  }

  _startAnimationLoop() {
    const animate = () => {
      requestAnimationFrame(animate);
      
      // Simulate reaction progress from backend snapshot
      if (this._lastSnapshot) {
        this._reactorTemperature = (this._lastSnapshot.reactor_temp || 0) / 500 * 0.9 + 0.1;
      }
      
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    animate();
  }

  update(snapshot) {
    this._lastSnapshot = snapshot;
  }
}
