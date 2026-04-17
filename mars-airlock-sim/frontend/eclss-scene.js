/**
 * ECLSS System (Environmental Control and Life Support System)
 * Procedural Three.js Scene for Underhill Base Dome
 *
 * Systems:
 * - Atmosphere: pressure regulation, composition control
 * - CO2 Scrubbing: Sabatier feedstock + lithium hydroxide backup
 * - O2 Generation: electrolysis water processing  
 * - Humidity Control: condensation loops, dehumidification
 * - Thermal Management: radiators, heat exchangers
 */

import * as THREE from "three";
import { OrbitControls } from "https://unpkg.com/three@0.161.0/examples/jsm/controls/OrbitControls.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const ECLSS_COLORS = {
  PRESSURE_VESSEL: 0x4a5a3e,
  PUMP_BODY: 0x5a7a3a,
  MOTOR: 0x6a5a3a,
  PIPE_COLD: 0x3a6aaa,
  PIPE_HOT: 0xaa3a3a,
  HEAT_EXCHANGER: 0x8a6a4a,
  FILTER: 0x7a7a6a,
  VALVE: 0xcc9944,
  RADIATOR_PANEL: 0x5a7a9a,
  SCRUBBER_BED: 0x9a7a5a,
  ACCUMULATOR: 0x5a5a7a,
  GAS_TANK: 0x4a6a7a,
};

const ECLSS_SCALE = 0.8;

// ---------------------------------------------------------------------------
// ECLSSScene
// ---------------------------------------------------------------------------
export class ECLSSScene {
  constructor(container) {
    this._container = container;
    this._lastSnapshot = null;

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0a0a15);

    this._initRenderer(container);
    this._initCameraAndControls(container);
    this._initLighting();
    this._initEnvironment();
    this._buildECLSSSystem();
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
    this.camera.position.set(8, 5, 8);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0, 2, 0);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 5;
    this.controls.maxDistance = 30;
    this.controls.autoRotate = true;
    this.controls.autoRotateSpeed = 0.5;

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
    // Main work lights (cool LED)
    const light1 = new THREE.DirectionalLight(0xaabbff, 1.0);
    light1.position.set(10, 8, 6);
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

    // Secondary fill light
    const light2 = new THREE.DirectionalLight(0xffddaa, 0.6);
    light2.position.set(-8, 6, -5);
    this.scene.add(light2);

    // Ambient
    this.scene.add(new THREE.AmbientLight(0x6a7a8a, 0.5));
  }

  _initEnvironment() {
    // Floor
    const floor = new THREE.Mesh(
      new THREE.PlaneGeometry(30, 30),
      new THREE.MeshLambertMaterial({ color: 0x2a3a3a })
    );
    floor.rotation.x = -Math.PI / 2;
    floor.position.y = -0.1;
    floor.receiveShadow = true;
    this.scene.add(floor);

    // Back wall
    const wall = new THREE.Mesh(
      new THREE.BoxGeometry(30, 12, 0.5),
      new THREE.MeshLambertMaterial({ color: 0x3a4a4a })
    );
    wall.position.z = -8;
    wall.position.y = 5;
    this.scene.add(wall);

    // Grid overlay on wall (visual reference)
    const gridHelper = new THREE.GridHelper(20, 20, 0x555577, 0x444466);
    gridHelper.position.z = -7.9;
    gridHelper.position.y = 0;
    this.scene.add(gridHelper);
  }

  _buildECLSSSystem() {
    const root = new THREE.Group();
    root.scale.setScalar(ECLSS_SCALE);
    this.scene.add(root);

    // ========== Atmosphere Subsystem ==========
    this._buildAtmosphereLoop(root, -2, 0, 0);

    // ========== CO2 Scrubber Subsystem ==========
    this._buildCO2Scrubber(root, 2, 0, 0);

    // ========== Thermal Management ==========
    this._buildThermalLoop(root, -2, 2, 2);

    // ========== O2 Generator (Electrolysis) ==========
    this._buildO2Generator(root, 2, 2, 2);

    // ========== Dust/Particle Filter ==========
    this._buildDustFilter(root, 0, 1, 3);

    // Connector piping between systems
    this._buildInterconnects(root);
  }

  // =========================================================================
  // Atmosphere Regulation Loop
  // =========================================================================
  _buildAtmosphereLoop(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Main pressure vessel (cylinder)
    const vesselGeo = new THREE.CylinderGeometry(0.8, 0.8, 2.0, 16);
    const vesselMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.PRESSURE_VESSEL,
      roughness: 0.6,
      metalness: 0.4,
    });
    const vessel = new THREE.Mesh(vesselGeo, vesselMat);
    vessel.castShadow = true;
    vessel.receiveShadow = true;
    group.add(vessel);

    // Hemispherical cap top
    const capGeo = new THREE.SphereGeometry(0.8, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2);
    const cap = new THREE.Mesh(capGeo, vesselMat);
    cap.position.y = 1.0;
    cap.castShadow = true;
    group.add(cap);

    // Pressure relief valve on top
    this._buildPressureValve(group, 0, 1.15, 0);

    // Inlet/outlet ports
    this._buildPort(group, 1.0, 0.5, 0, "Inlet");
    this._buildPort(group, -1.0, -0.5, 0, "Outlet");
    this._buildPort(group, 0, 0.8, 1.0, "Sensor");

    // Status indicator (glow based on pressure)
    const indicatorGeo = new THREE.TorusGeometry(0.35, 0.08, 8, 16);
    const indicatorMat = new THREE.MeshStandardMaterial({
      color: 0x00dd00,
      emissive: 0x00aa00,
      metalness: 0.8,
    });
    const indicator = new THREE.Mesh(indicatorGeo, indicatorMat);
    indicator.position.set(-1.2, 0.3, 0);
    indicator.rotation.z = Math.PI / 4;
    group.add(indicator);

    // Label
    this._addLabel(group, 0, -1.5, 0, "Pressure\nRegulator", 0x88ccff);
  }

  // =========================================================================
  // CO2 Scrubber System
  // =========================================================================
  _buildCO2Scrubber(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Scrubber bed (large bulky container)
    const scrubberGeo = new THREE.BoxGeometry(0.8, 1.6, 0.8);
    const scrubberMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.SCRUBBER_BED,
      roughness: 0.7,
      metalness: 0.2,
    });
    const scrubber = new THREE.Mesh(scrubberGeo, scrubberMat);
    scrubber.castShadow = true;
    scrubber.receiveShadow = true;
    group.add(scrubber);

    // Internal element visualization (lattice)
    const latticeGeo = new THREE.BoxGeometry(0.6, 1.4, 0.6);
    const latticeMat = new THREE.MeshStandardMaterial({
      color: 0xaa8a6a,
      wireframe: true,
      opacity: 0.3,
      transparent: true,
    });
    const lattice = new THREE.Mesh(latticeGeo, latticeMat);
    lattice.position.z = 0.05;
    group.add(lattice);

    // CO2 inlet manifold
    this._buildPort(group, 0.65, 0.8, 0, "CO₂ In");
    // Cleaned air outlet
    this._buildPort(group, -0.65, 0.5, 0, "O₂ Out");

    // Thermal manage coil
    this._buildCoil(group, 0, 0, -0.5, 0.4);

    // Bypass valve
    this._buildPressureValve(group, 0.4, 1.0, 0);

    // Status LED
    const statusGeo = new THREE.SphereGeometry(0.12, 8, 8);
    const statusMat = new THREE.MeshStandardMaterial({
      color: 0xffdd00,
      emissive: 0xffaa00,
    });
    const status = new THREE.Mesh(statusGeo, statusMat);
    status.position.set(-0.5, 1.0, 0.5);
    group.add(status);

    this._addLabel(group, 0, -1.2, 0, "CO₂ Scrubber", 0xffcc88);
  }

  // =========================================================================
  // Thermal Management Loop
  // =========================================================================
  _buildThermalLoop(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Heat exchanger core (ribbed design)
    const coreGeo = new THREE.BoxGeometry(1.2, 0.6, 0.4);
    const coreMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.HEAT_EXCHANGER,
      roughness: 0.5,
      metalness: 0.7,
    });
    const core = new THREE.Mesh(coreGeo, coreMat);
    core.castShadow = true;
    group.add(core);

    // Fins visualization
    for (let i = -3; i <= 3; i++) {
      const finGeo = new THREE.BoxGeometry(1.2, 0.08, 0.08);
      const fin = new THREE.Mesh(finGeo, coreMat);
      fin.position.y = i * 0.12;
      fin.castShadow = true;
      group.add(fin);
    }

    // Radiator panel (exposed to space)
    const radiatorGeo = new THREE.BoxGeometry(2.0, 0.3, 0.1);
    const radiatorMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.RADIATOR_PANEL,
      roughness: 0.3,
      metalness: 0.9,
    });
    const radiator = new THREE.Mesh(radiatorGeo, radiatorMat);
    radiator.position.z = 1.2;
    radiator.position.y = 0.3;
    radiator.castShadow = true;
    group.add(radiator);

    // Hot water inlet/outlet
    this._buildPort(group, -0.8, 0.3, 0, "Hot In");
    this._buildPort(group, 0.8, -0.3, 0, "Cold Out");

    this._addLabel(group, 0, -0.8, 0, "Thermal\nManagement", 0x88ffcc);
  }

  // =========================================================================
  // O2 Generation (Electrolysis)
  // =========================================================================
  _buildO2Generator(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Main electrolyzer unit
    const unitGeo = new THREE.BoxGeometry(0.7, 1.2, 0.7);
    const unitMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.PRESSURE_VESSEL,
      roughness: 0.6,
      metalness: 0.5,
    });
    const unit = new THREE.Mesh(unitGeo, unitMat);
    unit.castShadow = true;
    group.add(unit);

    // Electrolysis cell (internal detail)
    const cellGeo = new THREE.CylinderGeometry(0.25, 0.25, 0.8, 8);
    const cellMat = new THREE.MeshStandardMaterial({
      color: 0x4a7a9a,
      metalness: 0.7,
    });
    const cell = new THREE.Mesh(cellGeo, cellMat);
    cell.position.y = -0.2;
    group.add(cell);

    // Water inlet
    this._buildPort(group, -0.45, 0.6, 0, "H₂O In");
    // Oxygen outlet
    this._buildPort(group, 0.45, 0.6, 0, "O₂ Out");
    // Hydrogen outlet (small)
    this._buildSmallPort(group, 0, 0.8, 0.45, "H₂");

    // Power indicator
    const powerGeo = new THREE.SphereGeometry(0.1, 8, 8);
    const powerMat = new THREE.MeshStandardMaterial({
      color: 0xff6644,
      emissive: 0xaa3322,
    });
    const power = new THREE.Mesh(powerGeo, powerMat);
    power.position.set(-0.35, 0.9, 0);
    group.add(power);

    this._addLabel(group, 0, -0.9, 0, "O₂ Generator\n(Electrolysis)", 0xffaa88);
  }

  // =========================================================================
  // Dust/Particulate Filter
  // =========================================================================
  _buildDustFilter(parent, x, y, z) {
    const group = new THREE.Group();
    group.position.set(x, y, z);
    parent.add(group);

    // Main filter cartridge
    const cartridgeGeo = new THREE.CylinderGeometry(0.4, 0.4, 1.0, 8);
    const cartridgeMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.FILTER,
      roughness: 0.8,
      metalness: 0.3,
    });
    const cartridge = new THREE.Mesh(cartridgeGeo, cartridgeMat);
    cartridge.castShadow = true;
    group.add(cartridge);

    // Filter media ripples
    const rippleCount = 5;
    for (let i = 0; i < rippleCount; i++) {
      const rippleGeo = new THREE.CylinderGeometry(0.35, 0.35, 0.1, 16);
      const ripple = new THREE.Mesh(rippleGeo, cartridgeMat);
      ripple.position.y = -0.4 + (i * 0.5);
      group.add(ripple);
    }

    // Pressure drop indicator gauge
    const gaugeGeo = new THREE.TorusGeometry(0.25, 0.05, 8, 16);
    const gaugeMat = new THREE.MeshStandardMaterial({
      color: 0xcccccc,
      metalness: 0.9,
    });
    const gauge = new THREE.Mesh(gaugeGeo, gaugeMat);
    gauge.position.set(0.6, 0.2, 0);
    group.add(gauge);

    // Contamination warning LED
    const warnGeo = new THREE.SphereGeometry(0.08, 8, 8);
    const warnMat = new THREE.MeshStandardMaterial({
      color: 0x22dd22,
      emissive: 0x11aa11,
    });
    const warn = new THREE.Mesh(warnGeo, warnMat);
    warn.position.set(0.6, 0.5, 0);
    group.add(warn);

    // Air inlet/outlet
    this._buildPort(group, 0, 0.55, 0, "In");
    this._buildPort(group, 0, -0.55, 0, "Out");

    this._addLabel(group, 0, -0.8, 0, "Filter", 0xcccccc);
  }

  // =========================================================================
  // Interconnect Piping
  // =========================================================================
  _buildInterconnects(parent) {
    // High-pressure oxygen line (hot red)
    this._buildPipeLine(parent, [
      [-2, 0, 0],
      [-1, 0.5, 0.5],
      [0, 1, 1],
      [1, 1, 1],
    ], 0.08, ECLSS_COLORS.PIPE_HOT);

    // Cold water return (cool blue)
    this._buildPipeLine(parent, [
      [2, 2, 2],
      [1.5, 1.5, 1.5],
      [0, 1, 1],
      [-1, 0.5, 0.5],
    ], 0.06, ECLSS_COLORS.PIPE_COLD);

    // CO2 vent line
    this._buildPipeLine(parent, [
      [2, 0, 0],
      [1, 1.5, 1],
      [0, 2.5, 2],
    ], 0.05, 0x9a4a4a);
  }

  // =========================================================================
  // Helper Methods
  // =========================================================================

  _buildPressureValve(group, x, y, z) {
    const valveGeo = new THREE.CylinderGeometry(0.15, 0.1, 0.3, 6);
    const valveMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.VALVE,
      roughness: 0.4,
      metalness: 0.8,
    });
    const valve = new THREE.Mesh(valveGeo, valveMat);
    valve.position.set(x, y, z);
    valve.rotation.z = Math.PI / 4;
    valve.castShadow = true;
    group.add(valve);

    // Valve handle
    const handleGeo = new THREE.BoxGeometry(0.08, 0.3, 0.08);
    const handleMat = new THREE.MeshStandardMaterial({
      color: 0x8a6a4a,
      metalness: 0.9,
    });
    const handle = new THREE.Mesh(handleGeo, handleMat);
    handle.position.set(x + 0.15, y + 0.15, z);
    handle.castShadow = true;
    group.add(handle);
  }

  _buildPort(group, x, y, z, label) {
    const portGeo = new THREE.CylinderGeometry(0.12, 0.12, 0.25, 6);
    const portMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.VALVE,
      roughness: 0.5,
      metalness: 0.7,
    });
    const port = new THREE.Mesh(portGeo, portMat);
    port.position.set(x, y, z);
    port.castShadow = true;
    group.add(port);

    // Port connector
    const connGeo = new THREE.SphereGeometry(0.1, 6, 6);
    const conn = new THREE.Mesh(connGeo, portMat);
    conn.position.set(x, y, z);
    group.add(conn);

    // Small label
    this._addSmallLabel(group, x, y + 0.3, z, label, 0xffbb99);
  }

  _buildSmallPort(group, x, y, z, label) {
    const portGeo = new THREE.CylinderGeometry(0.08, 0.08, 0.15, 6);
    const portMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.VALVE,
      metalness: 0.8,
    });
    const port = new THREE.Mesh(portGeo, portMat);
    port.position.set(x, y, z);
    port.castShadow = true;
    group.add(port);

    this._addSmallLabel(group, x + 0.2, y, z, label, 0xffbb99);
  }

  _buildCoil(group, x, y, z, radius) {
    const coilGeo = new THREE.TorusGeometry(radius, 0.04, 6, 16);
    const coilMat = new THREE.MeshStandardMaterial({
      color: ECLSS_COLORS.PIPE_COLD,
      metalness: 0.9,
    });
    const coil = new THREE.Mesh(coilGeo, coilMat);
    coil.position.set(x, y, z);
    coil.rotation.x = Math.PI / 2;
    group.add(coil);
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
    }
  }

  _addLabel(group, x, y, z, text, color) {
    // Visual marker
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

  _addSmallLabel(group, x, y, z, text, color) {
    // Small dot label
    const dotGeo = new THREE.SphereGeometry(0.08, 6, 6);
    const dotMat = new THREE.MeshStandardMaterial({
      color: color,
      metalness: 0.5,
    });
    const dot = new THREE.Mesh(dotGeo, dotMat);
    dot.position.set(x, y, z);
    group.add(dot);
  }

  _startAnimationLoop() {
    const animate = () => {
      requestAnimationFrame(animate);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    };
    animate();
  }

  update(snapshot) {
    this._lastSnapshot = snapshot;
  }
}
