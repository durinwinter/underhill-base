/**
 * Underhill Mars Base - Unified Procedural Three.js Scene
 * 
 * This scene builds a full Martian base including the central hub, 
 * habitat domes, solar arrays, and the airlock simulator.
 * 
 * Driven by backend simulation snapshots via update(snapshot).
 */

import * as THREE from "three";
import { OrbitControls } from "https://unpkg.com/three@0.161.0/examples/jsm/controls/OrbitControls.js";

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
const VIEWPORT_HEIGHT = 800; // Larger unified viewport
const HABITAT_PRESSURE_PA = 101325;
const MARS_PRESSURE_PA = 700;

// Mars palette
const COL_MARS_GROUND = 0x8b5a3a;
const COL_SKY_TOP = 0xb5654a;
const COL_SKY_HORIZON = 0xd4a56a;
const COL_HULL_BASE = 0x6a7a5a;
const COL_HULL_HABITAT = 0x8a9a7a;
const COL_HULL_RIB = 0x4d5e40;
const COL_HATCH = 0x2a3a20;
const COL_PIPE = 0x5a6a4a;
const COL_VALVE_BODY = 0x4a5a3e;
const COL_HANDWHEEL = 0xaa4444;
const COL_PUMP_HOUSING = 0x5a7a3a;
const COL_LOCK_LOCKED = 0x00cc00;
const COL_LOCK_UNLOCKED = 0xffaa00;
const COL_DOME_GLASS = 0x88ccff;

const DEFAULT_BASE_LAYOUT = {
  hub: { id: "CoreHub", position: [0, -2.0, -25] },
  nodes: [
    { id: "Eclss", type: "SubsystemPod", name: "ECLSS MODULE", color: 0x4a7a9a, position: [-20, -2.0, -15], bind: "eclss" },
    { id: "Sabatier", type: "SubsystemPod", name: "SABATIER REACTOR", color: 0x9a4a4a, position: [20, -2.0, -15], bind: "sabatier" },
    { id: "LanderWest", type: "HabitatLander", name: "HAB LANDER W", position: [-24, -2.0, -30] },
    { id: "LanderEast", type: "HabitatLander", name: "HAB LANDER E", position: [-8, -2.0, -30] },
    { id: "ServiceWest", type: "ServiceModule", name: "SERVICE WEST", position: [-31, -2.0, -19], rotationY: 0.2 },
    { id: "ServiceMid", type: "ServiceModule", name: "SERVICE MID", position: [-16, -2.0, -17], rotationY: -0.1 },
    { id: "GreenhouseWest", type: "GreenhouseTube", name: "GREENHOUSE A", position: [-46, -2.0, -24], rotationY: 0.0, length: 28 },
    { id: "GreenhouseSouth", type: "GreenhouseTube", name: "GREENHOUSE B", position: [8, -2.0, -40], rotationY: -0.2, length: 24 },
    { id: "PowerDist", type: "PowerDistribution", name: "POWER DIST", position: [35, -2.0, -11] },
    { id: "FuelPlant", type: "FuelProcessing", name: "FUEL PROCESS", position: [42, -2.0, -18], rotationY: 0.8 },
    { id: "FuelTank", type: "StorageSphere", name: "FUEL", position: [56, -2.0, -11], radius: 3.8 },
    { id: "OxTank", type: "StorageSphere", name: "OXIDIZER", position: [52, -2.0, -20], radius: 4.0 },
    { id: "Reactor", type: "NuclearReactor", name: "NUCLEAR", position: [52, -2.0, -4], rotationY: 0.4 },
    { id: "Comms", type: "CommunicationsArray", name: "COMMS", position: [-52, -2.0, -21], rotationY: 0.5 },
    { id: "Fueling", type: "FuelingStation", name: "FUELING", position: [49, -2.0, -30], rotationY: 1.2 },
  ],
  links: [
    { from: "CoreHub", to: "Eclss", kind: "Tunnel" },
    { from: "CoreHub", to: "Sabatier", kind: "Tunnel" },
    { from: "CoreHub", to: "LanderWest", kind: "Tunnel" },
    { from: "CoreHub", to: "LanderEast", kind: "Tunnel" },
    { from: "LanderWest", to: "ServiceWest", kind: "Tunnel" },
    { from: "LanderEast", to: "ServiceMid", kind: "Tunnel" },
    { from: "ServiceWest", to: "GreenhouseWest", kind: "Tunnel" },
    { from: "CoreHub", to: "GreenhouseSouth", kind: "Tunnel" },
    { from: "CoreHub", to: "PowerDist", kind: "TrenchLine" },
    { from: "PowerDist", to: "Reactor", kind: "TrenchLine" },
    { from: "PowerDist", to: "FuelPlant", kind: "TrenchLine" },
    { from: "FuelPlant", to: "FuelTank", kind: "TrenchLine" },
    { from: "FuelPlant", to: "OxTank", kind: "TrenchLine" },
    { from: "FuelPlant", to: "Fueling", kind: "TrenchLine" },
    { from: "ServiceWest", to: "Comms", kind: "TrenchLine" },
    { from: "CoreHub", to: "AirlockEntry", kind: "Tunnel", toPosition: [0, -2.0, -8] },
  ],
  solarFarm: { enabled: false, position: [-40, -1.8, -40], rows: 0, cols: 0, spacingX: 6, spacingZ: 8 },
};

// ---------------------------------------------------------------------------
// UnderhillBaseScene
// ---------------------------------------------------------------------------
export class UnderhillBaseScene {
  constructor(container) {
    this._container = container;
    this._lastSnapshot = null;
    this._pumpSpeed = 0;
    this._proceduralScale = 1.0;
    this._layoutDebugEnabled = false;

    this.scene = new THREE.Scene();
    this._root = new THREE.Group();
    this.scene.add(this._root);

    this._initRenderer(container);
    this._initCameraAndControls(container);
    this._initLighting();
    this._initMarsEnvironment();

    // Build the Base
    this._buildMainBase(DEFAULT_BASE_LAYOUT);
    void this._loadBaseLayout("./layout/base-layout.json");
    this._initAirlockChamber();
    this._initDoors();
    this._initLockIndicators();
    this._initPipingAndValves();
    this._initVacuumPump();

    this._initParticleSystems();
    this._initLabels();
    this._startAnimationLoop();
  }

  update(snapshot) {
    this._lastSnapshot = snapshot;
  }

  setLayoutDebug(enabled) {
    this._layoutDebugEnabled = Boolean(enabled);
    this._rebuildLayoutDebug();
  }

  resize() {
    if (!this._container || !this.renderer) return;
    const w = this._container.clientWidth;
    const h = this._container.clientHeight || VIEWPORT_HEIGHT;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
  }

  dispose() {
    window.removeEventListener("resize", this._onResize);
    this.renderer.dispose();
  }

  // =========================================================================
  // Renderer
  // =========================================================================
  _initRenderer(container) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setSize(container.clientWidth, container.clientHeight || VIEWPORT_HEIGHT);
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.1;
    container.innerHTML = "";
    container.appendChild(this.renderer.domElement);

    this._onResize = () => this.resize();
    window.addEventListener("resize", this._onResize);
  }

  // =========================================================================
  // Camera + OrbitControls
  // =========================================================================
  _initCameraAndControls() {
    const w = this._container.clientWidth;
    const h = this._container.clientHeight || VIEWPORT_HEIGHT;
    this.camera = new THREE.PerspectiveCamera(50, w / h, 0.1, 500);
    this.camera.position.set(25, 15, 45);
    this.camera.lookAt(0, 5, 0);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.target.set(0, 2, 0);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 10;
    this.controls.maxDistance = 150;
    this.controls.maxPolarAngle = Math.PI * 0.45;
    this.controls.autoRotate = true;
    this.controls.autoRotateSpeed = 0.2;

    let idleTimer = null;
    this.controls.addEventListener("start", () => {
      this.controls.autoRotate = false;
      clearTimeout(idleTimer);
    });
    this.controls.addEventListener("end", () => {
      idleTimer = setTimeout(() => { this.controls.autoRotate = true; }, 15000);
    });
  }

  // =========================================================================
  // Lighting
  // =========================================================================
  _initLighting() {
    const sun = new THREE.DirectionalLight(0xffddcc, 1.5);
    sun.position.set(50, 40, 30);
    this.scene.add(sun);

    this.scene.add(new THREE.AmbientLight(0xc49060, 0.4));
    this.scene.add(new THREE.HemisphereLight(0xd4956b, 0xc2784a, 0.3));
  }

  // =========================================================================
  // Mars Environment
  // =========================================================================
  _initMarsEnvironment() {
    // Sky
    const skyGeo = new THREE.SphereGeometry(300, 32, 16);
    const skyMat = new THREE.ShaderMaterial({
      side: THREE.BackSide,
      depthWrite: false,
      uniforms: {
        topColor: { value: new THREE.Color(COL_SKY_TOP) },
        bottomColor: { value: new THREE.Color(COL_SKY_HORIZON) },
      },
      vertexShader: `
        varying vec3 vWorldPos;
        void main() {
          vWorldPos = (modelMatrix * vec4(position, 1.0)).xyz;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }`,
      fragmentShader: `
        uniform vec3 topColor;
        uniform vec3 bottomColor;
        varying vec3 vWorldPos;
        void main() {
          float h = normalize(vWorldPos).y;
          float t = smoothstep(-0.2, 0.6, h);
          gl_FragColor = vec4(mix(bottomColor, topColor, t), 1.0);
        }`,
    });
    this.scene.add(new THREE.Mesh(skyGeo, skyMat));

    // Ground
    const groundGeo = new THREE.PlaneGeometry(1000, 1000, 100, 100);
    groundGeo.rotateX(-Math.PI / 2);
    const posAttr = groundGeo.attributes.position;
    const colors = new Float32Array(posAttr.count * 3);
    const baseCol = new THREE.Color(COL_MARS_GROUND);
    for (let i = 0; i < posAttr.count; i++) {
      const x = posAttr.getX(i);
      const z = posAttr.getZ(i);
      const d = Math.sqrt(x * x + z * z);
      // Flatter near the base
      const noise = (Math.random() - 0.5) * (d > 50 ? 2.0 : 0.2);
      posAttr.setY(i, noise);

      const v = 0.8 + Math.random() * 0.4;
      colors[i * 3] = baseCol.r * v;
      colors[i * 3 + 1] = baseCol.g * v;
      colors[i * 3 + 2] = baseCol.b * v;
    }
    groundGeo.setAttribute("color", new THREE.BufferAttribute(colors, 3));
    groundGeo.computeVertexNormals();
    const ground = new THREE.Mesh(groundGeo, new THREE.MeshLambertMaterial({ vertexColors: true }));
    ground.position.y = -2.0;
    this.scene.add(ground);

    this._initDust();
  }

  _initDust() {
    const count = 1000;
    const positions = new Float32Array(count * 3);
    this._dustVelocities = new Float32Array(count * 3);
    for (let i = 0; i < count; i++) {
      positions[i * 3] = (Math.random() - 0.5) * 150;
      positions[i * 3 + 1] = Math.random() * 20;
      positions[i * 3 + 2] = (Math.random() - 0.5) * 150;
      this._dustVelocities[i * 3] = 0.5 + Math.random() * 1.0;
      this._dustVelocities[i * 3 + 1] = (Math.random() - 0.5) * 0.1;
      this._dustVelocities[i * 3 + 2] = (Math.random() - 0.5) * 0.2;
    }
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    const mat = new THREE.PointsMaterial({
      color: 0xc4956a, size: 0.1, transparent: true, opacity: 0.2, depthWrite: false,
    });
    this._dustPoints = new THREE.Points(geo, mat);
    this.scene.add(this._dustPoints);
  }

  _updateDust(dt) {
    const pos = this._dustPoints.geometry.attributes.position;
    const vel = this._dustVelocities;
    for (let i = 0; i < pos.count; i++) {
      let x = pos.getX(i) + vel[i * 3] * dt;
      let y = pos.getY(i) + vel[i * 3 + 1] * dt;
      let z = pos.getZ(i) + vel[i * 3 + 2] * dt;
      if (x > 75) x = -75;
      if (y < 0) y = 20;
      if (y > 20) y = 0;
      if (Math.abs(z) > 75) z = -z;
      pos.setXYZ(i, x, y, z);
    }
    pos.needsUpdate = true;
  }

  // =========================================================================
  // Main Base Construction
  // =========================================================================
  _buildMainBase(layout) {
    this._activeBaseLayout = layout;

    if (this._baseInfraGroup) {
      this._root.remove(this._baseInfraGroup);
    }
    if (this._layoutDebugGroup) {
      this._root.remove(this._layoutDebugGroup);
    }

    this._baseInfraGroup = new THREE.Group();
    this._baseInfraGroup.name = "base-infra";
    this._root.add(this._baseInfraGroup);
    this._modulesById = new Map();

    const hubPos = new THREE.Vector3(...(layout.hub?.position || [0, -2.0, -25]));
    const hub = this._createCoreHub();
    hub.position.copy(hubPos);
    this._baseInfraGroup.add(hub);
    this._modulesById.set(layout.hub?.id || "CoreHub", hub);

    this._eclssPod = null;
    this._sabatierPod = null;

    for (const node of layout.nodes || []) {
      const module = this._createModuleFromNode(node);
      if (!module) continue;
      module.position.set(...(node.position || [0, 0, 0]));
      module.rotation.y = node.rotationY || 0;
      this._baseInfraGroup.add(module);
      this._modulesById.set(node.id, module);
      if (node.bind === "eclss") this._eclssPod = module;
      if (node.bind === "sabatier") this._sabatierPod = module;
    }

    for (const link of layout.links || []) {
      this._addLayoutLink(link);
    }

    this._buildSolarFarm(layout.solarFarm || {});
    this._rebuildLayoutDebug();
  }

  async _loadBaseLayout(url) {
    try {
      const response = await fetch(url, { cache: "no-store" });
      if (!response.ok) return;
      const layout = await response.json();
      if (layout && typeof layout === "object") {
        this._buildMainBase(layout);
      }
    } catch (_) {
      // Keep default in-scene layout when external file is unavailable.
    }
  }

  _createCoreHub() {
    const hub = new THREE.Group();
    const domeGeo = new THREE.SphereGeometry(15, 32, 16, 0, Math.PI * 2, 0, Math.PI / 2);
    const domeMat = new THREE.MeshStandardMaterial({
      color: COL_DOME_GLASS,
      transparent: true,
      opacity: 0.4,
      metalness: 0.8,
      roughness: 0.1,
    });
    hub.add(new THREE.Mesh(domeGeo, domeMat));
    hub.add(
      new THREE.Mesh(
        domeGeo,
        new THREE.MeshBasicMaterial({
          color: 0xffffff,
          wireframe: true,
          transparent: true,
          opacity: 0.1,
        }),
      ),
    );
    const hubBase = new THREE.Mesh(
      new THREE.CylinderGeometry(14, 14, 2, 32),
      new THREE.MeshStandardMaterial({ color: 0x5a5a5a }),
    );
    hubBase.position.y = 0.5;
    hub.add(hubBase);
    return hub;
  }

  _createModuleFromNode(node) {
    const type = node.type || "ServiceModule";
    switch (type) {
      case "SubsystemPod":
        return this._createSubsystemPod(node.name || "SUBSYSTEM", node.color || 0x7a7a7a);
      case "HabitatLander":
        return this._createHabitatLander(node.name || "HAB LANDER");
      case "GreenhouseTube":
        return this._createGreenhouseTube(node.name || "GREENHOUSE", node.length || 20);
      case "ServiceModule":
        return this._createServiceModule(node.name || "SERVICE");
      case "PowerDistribution":
        return this._createPowerDistribution(node.name || "POWER");
      case "FuelProcessing":
        return this._createFuelProcessing(node.name || "FUEL PROCESS");
      case "StorageSphere":
        return this._createStorageSphere(node.name || "STORAGE", node.radius || 3.5);
      case "NuclearReactor":
        return this._createNuclearReactor(node.name || "REACTOR");
      case "CommunicationsArray":
        return this._createCommunicationsArray(node.name || "COMMS");
      case "FuelingStation":
        return this._createFuelingStation(node.name || "FUELING");
      default:
        return this._createServiceModule(node.name || "MODULE");
    }
  }

  _createSubsystemPod(name, color) {
    const pod = new THREE.Group();
    const body = new THREE.Mesh(
      new THREE.CylinderGeometry(5, 5, 8, 16),
      new THREE.MeshStandardMaterial({ color: COL_HULL_HABITAT }),
    );
    body.position.y = 4.0;
    pod.add(body);

    const cap = new THREE.Mesh(
      new THREE.SphereGeometry(5, 16, 8, 0, Math.PI * 2, 0, Math.PI / 2),
      new THREE.MeshStandardMaterial({ color, transparent: true, opacity: 0.5 }),
    );
    cap.position.y = 8.0;
    pod.add(cap);
    pod.userData.indicatorCap = cap;

    pod.add(this._createTextSprite(name, 0, 10, 0));
    return pod;
  }

  _createHabitatLander(name) {
    const g = new THREE.Group();
    const shell = new THREE.Mesh(
      new THREE.SphereGeometry(6.2, 20, 14, 0, Math.PI * 2, 0, Math.PI / 2),
      new THREE.MeshStandardMaterial({
        color: 0x14171a,
        metalness: 0.6,
        roughness: 0.35,
      }),
    );
    shell.position.y = 2.7;
    g.add(shell);

    const base = new THREE.Mesh(
      new THREE.CylinderGeometry(5.5, 5.5, 1.5, 20),
      new THREE.MeshStandardMaterial({ color: 0x666a6f }),
    );
    base.position.y = 0.6;
    g.add(base);

    g.add(this._createTextSprite(name, 0, 8, 0));
    return g;
  }

  _createGreenhouseTube(name, length) {
    const g = new THREE.Group();
    const shell = new THREE.Mesh(
      new THREE.CylinderGeometry(2.8, 2.8, length, 20, 1, true),
      new THREE.MeshStandardMaterial({ color: 0xdce6d0, metalness: 0.25, roughness: 0.7 }),
    );
    shell.rotation.z = Math.PI / 2;
    shell.position.y = 1.5;
    g.add(shell);

    const canopy = new THREE.Mesh(
      new THREE.CylinderGeometry(2.7, 2.7, length - 0.6, 20, 1, true, 0, Math.PI),
      new THREE.MeshStandardMaterial({ color: 0x7abf89, transparent: true, opacity: 0.55 }),
    );
    canopy.rotation.z = Math.PI / 2;
    canopy.position.y = 2.2;
    g.add(canopy);

    g.add(this._createTextSprite(name, 0, 6.5, 0));
    return g;
  }

  _createServiceModule(name) {
    const g = new THREE.Group();
    const body = new THREE.Mesh(
      new THREE.CapsuleGeometry(1.5, 5, 8, 14),
      new THREE.MeshStandardMaterial({ color: 0xbfc7b6, metalness: 0.3, roughness: 0.7 }),
    );
    body.rotation.z = Math.PI / 2;
    body.position.y = 1.1;
    g.add(body);
    g.add(this._createTextSprite(name, 0, 4.5, 0));
    return g;
  }

  _createPowerDistribution(name) {
    const g = new THREE.Group();
    const mast = new THREE.Mesh(
      new THREE.CylinderGeometry(0.4, 0.5, 6, 12),
      new THREE.MeshStandardMaterial({ color: 0x6f7a6a }),
    );
    mast.position.y = 3.0;
    g.add(mast);

    const hub = new THREE.Mesh(
      new THREE.CylinderGeometry(1.5, 1.5, 0.6, 16),
      new THREE.MeshStandardMaterial({ color: 0x9fa89a }),
    );
    hub.position.y = 6.3;
    g.add(hub);
    g.add(this._createTextSprite(name, 0, 8.5, 0));
    return g;
  }

  _createFuelProcessing(name) {
    const g = new THREE.Group();
    const base = new THREE.Mesh(
      new THREE.BoxGeometry(6, 3, 4),
      new THREE.MeshStandardMaterial({ color: 0x9a9a95 }),
    );
    base.position.y = 1.5;
    g.add(base);
    const stack = new THREE.Mesh(
      new THREE.CylinderGeometry(0.45, 0.45, 4.5, 12),
      new THREE.MeshStandardMaterial({ color: 0x8b8b85 }),
    );
    stack.position.set(-1.4, 4.5, 0);
    g.add(stack);
    g.add(this._createTextSprite(name, 0, 6.5, 0));
    return g;
  }

  _createStorageSphere(name, radius) {
    const g = new THREE.Group();
    const tank = new THREE.Mesh(
      new THREE.SphereGeometry(radius, 18, 14),
      new THREE.MeshStandardMaterial({ color: 0xb8bdc8, metalness: 0.45, roughness: 0.35 }),
    );
    tank.position.y = radius;
    g.add(tank);
    g.add(this._createTextSprite(name, 0, radius * 2.3, 0));
    return g;
  }

  _createNuclearReactor(name) {
    const g = new THREE.Group();
    const core = new THREE.Mesh(
      new THREE.CylinderGeometry(2.4, 2.8, 4.8, 16),
      new THREE.MeshStandardMaterial({ color: 0x6e7278, metalness: 0.5, roughness: 0.45 }),
    );
    core.position.y = 2.4;
    g.add(core);
    for (let i = 0; i < 8; i++) {
      const fin = new THREE.Mesh(
        new THREE.BoxGeometry(0.35, 0.12, 2.6),
        new THREE.MeshStandardMaterial({ color: 0x828892 }),
      );
      const a = (i / 8) * Math.PI * 2;
      fin.position.set(Math.cos(a) * 2.7, 3.9, Math.sin(a) * 2.7);
      fin.rotation.y = a;
      g.add(fin);
    }
    g.add(this._createTextSprite(name, 0, 7, 0));
    return g;
  }

  _createCommunicationsArray(name) {
    const g = new THREE.Group();
    const mast = new THREE.Mesh(
      new THREE.CylinderGeometry(0.2, 0.25, 5.2, 10),
      new THREE.MeshStandardMaterial({ color: 0x9aa0a8 }),
    );
    mast.position.y = 2.6;
    g.add(mast);
    const dish = new THREE.Mesh(
      new THREE.ConeGeometry(1.4, 0.7, 16, 1, true),
      new THREE.MeshStandardMaterial({ color: 0xe7ebef, side: THREE.DoubleSide }),
    );
    dish.rotation.x = Math.PI / 2;
    dish.position.y = 5.2;
    g.add(dish);
    g.add(this._createTextSprite(name, 0, 7.1, 0));
    return g;
  }

  _createFuelingStation(name) {
    const g = new THREE.Group();
    const pad = new THREE.Mesh(
      new THREE.CylinderGeometry(2.2, 2.2, 0.25, 24),
      new THREE.MeshStandardMaterial({ color: 0x5f6460 }),
    );
    pad.position.y = 0.12;
    g.add(pad);
    const nozzle = new THREE.Mesh(
      new THREE.CylinderGeometry(0.2, 0.2, 2.2, 10),
      new THREE.MeshStandardMaterial({ color: 0x959a98 }),
    );
    nozzle.position.set(0.8, 1.2, 0);
    g.add(nozzle);
    g.add(this._createTextSprite(name, 0, 3.6, 0));
    return g;
  }

  _addLayoutLink(link) {
    const from = this._modulesById.get(link.from);
    const fromPos = from ? from.position : new THREE.Vector3();
    const toPos = link.toPosition
      ? new THREE.Vector3(...link.toPosition)
      : this._modulesById.get(link.to)?.position;
    if (!toPos) return;
    this._addConnector(fromPos, toPos, link.kind || "Tunnel");
  }

  _addConnector(posA, posB, kind) {
    const dir = new THREE.Vector3().subVectors(posB, posA);
    const length = dir.length();
    if (length < 0.5) return;

    const isTrench = kind === "TrenchLine";
    const radius = isTrench ? 0.45 : 2.0;
    const yOffset = isTrench ? -1.7 : -1.0;
    const material = new THREE.MeshStandardMaterial({
      color: isTrench ? 0x7f857d : 0x444444,
      metalness: isTrench ? 0.35 : 0.7,
      roughness: isTrench ? 0.7 : 0.4,
    });
    const connector = new THREE.Mesh(
      new THREE.CylinderGeometry(radius, radius, length, isTrench ? 10 : 12),
      material,
    );
    connector.position.copy(posA).addScaledVector(dir, 0.5);
    connector.position.y = yOffset;
    connector.quaternion.setFromUnitVectors(
      new THREE.Vector3(0, 1, 0),
      dir.clone().normalize(),
    );
    this._baseInfraGroup.add(connector);
  }

  _rebuildLayoutDebug() {
    if (this._layoutDebugGroup) {
      this._root.remove(this._layoutDebugGroup);
    }
    this._layoutDebugGroup = new THREE.Group();
    this._layoutDebugGroup.name = "layout-debug";
    this._layoutDebugGroup.visible = this._layoutDebugEnabled;
    this._root.add(this._layoutDebugGroup);

    if (!this._layoutDebugEnabled || !this._activeBaseLayout) return;

    const nodeMarkerGeo = new THREE.SphereGeometry(0.35, 10, 8);
    const hubMat = new THREE.MeshBasicMaterial({ color: 0x00d4ff });
    const nodeMat = new THREE.MeshBasicMaterial({ color: 0xffd24d });

    const addNodeDebug = (id, pos, colorMat) => {
      const marker = new THREE.Mesh(nodeMarkerGeo, colorMat);
      marker.position.copy(pos).add(new THREE.Vector3(0, 0.8, 0));
      this._layoutDebugGroup.add(marker);
      const label = this._createTextSprite(id, pos.x, pos.y + 2.3, pos.z);
      label.scale.multiplyScalar(0.45);
      this._layoutDebugGroup.add(label);
    };

    const hubId = this._activeBaseLayout.hub?.id || "CoreHub";
    const hubPos = this._modulesById.get(hubId)?.position || new THREE.Vector3();
    addNodeDebug(hubId, hubPos, hubMat);

    for (const node of this._activeBaseLayout.nodes || []) {
      const p = this._modulesById.get(node.id)?.position;
      if (!p) continue;
      addNodeDebug(node.id, p, nodeMat);
    }

    for (const link of this._activeBaseLayout.links || []) {
      const fromPos = this._modulesById.get(link.from)?.position;
      const toPos = link.toPosition
        ? new THREE.Vector3(...link.toPosition)
        : this._modulesById.get(link.to)?.position;
      if (!fromPos || !toPos) continue;

      const points = [fromPos.clone().setY(-0.4), toPos.clone().setY(-0.4)];
      const geometry = new THREE.BufferGeometry().setFromPoints(points);
      const material = new THREE.LineBasicMaterial({
        color: link.kind === "TrenchLine" ? 0x57c1ff : 0xff7f50,
      });
      this._layoutDebugGroup.add(new THREE.Line(geometry, material));

      const mid = fromPos.clone().lerp(toPos, 0.5);
      const label = this._createTextSprite(`${link.from}->${link.to || "Point"}`, mid.x, mid.y + 1.4, mid.z);
      label.scale.multiplyScalar(0.32);
      this._layoutDebugGroup.add(label);
    }
  }

  _buildSolarFarm(config) {
    if (config.enabled === false) return;

    const farm = new THREE.Group();
    const position = config.position || [-40, -1.8, -40];
    const rows = config.rows ?? 5;
    const cols = config.cols ?? 4;
    if (rows <= 0 || cols <= 0) return;
    const spacingX = config.spacingX || 6;
    const spacingZ = config.spacingZ || 8;

    farm.position.set(position[0], position[1], position[2]);
    this._baseInfraGroup.add(farm);

    const panelGeo = new THREE.PlaneGeometry(3, 5);
    const panelMat = new THREE.MeshStandardMaterial({
      color: 0x1a1a2a,
      metalness: 0.9,
      roughness: 0.1,
    });
    for (let x = 0; x < rows; x++) {
      for (let z = 0; z < cols; z++) {
        const panel = new THREE.Mesh(panelGeo, panelMat);
        panel.position.set(x * spacingX, 2, z * spacingZ);
        panel.rotation.x = -Math.PI / 3;
        farm.add(panel);

        const support = new THREE.Mesh(
          new THREE.CylinderGeometry(0.1, 0.1, 3),
          new THREE.MeshStandardMaterial({ color: 0x333333 }),
        );
        support.position.set(x * spacingX, 0, z * spacingZ);
        farm.add(support);
      }
    }
  }


  // =========================================================================
  // Airlock Simulator (Original logic integrated)
  // =========================================================================
  _initAirlockChamber() {
    this._airlockGroup = new THREE.Group();
    this._airlockGroup.position.set(0, 0.5, -4); // Moved closer to hub
    this._airlockGroup.rotation.y = Math.PI / 2; // Rotate 90 deg to align with tunnel

    const chamberGeo = new THREE.CylinderGeometry(1.5, 1.5, 4, 32, 1, true);
    chamberGeo.rotateZ(Math.PI / 2);
    this._chamberMat = new THREE.MeshStandardMaterial({
      color: COL_HULL_BASE, metalness: 0.5, roughness: 0.5, side: THREE.DoubleSide,
    });
    this._airlockGroup.add(new THREE.Mesh(chamberGeo, this._chamberMat));

    // Support legs
    const legMat = new THREE.MeshStandardMaterial({ color: 0x5a5a4a, metalness: 0.6, roughness: 0.5 });
    for (const x of [-1.2, 1.2]) {
      for (const z of [-1.2, 1.2]) {
        const leg = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.09, 2.0, 8), legMat);
        leg.position.set(x, -2.0, z);
        this._airlockGroup.add(leg);
      }
    }

    // Pressure fog
    const fogGeo = new THREE.CylinderGeometry(1.35, 1.35, 3.6, 16);
    fogGeo.rotateZ(Math.PI / 2);
    this._pressureFogMat = new THREE.MeshBasicMaterial({
      color: 0xc8d8b8, transparent: true, opacity: 0.0, depthWrite: false,
    });
    this._airlockGroup.add(new THREE.Mesh(fogGeo, this._pressureFogMat));

    this._root.add(this._airlockGroup);
  }

  _initDoors() {
    const hatchRadius = 1.3;
    const hatchThickness = 0.1;

    // Inner hatch
    this._innerPivot = new THREE.Group();
    this._innerPivot.position.set(-2.0, 1.5, 0);
    const innerHatch = this._createHatchMesh(hatchRadius, hatchThickness);
    innerHatch.position.y = -hatchRadius;
    this._innerPivot.add(innerHatch);
    this._airlockGroup.add(this._innerPivot);

    // Outer hatch
    this._outerPivot = new THREE.Group();
    this._outerPivot.position.set(2.0, 1.5, 0);
    const outerHatch = this._createHatchMesh(hatchRadius, hatchThickness);
    outerHatch.position.y = -hatchRadius;
    this._outerPivot.add(outerHatch);
    this._airlockGroup.add(this._outerPivot);
  }

  _createHatchMesh(radius, thickness) {
    const g = new THREE.Group();
    const disc = new THREE.Mesh(
      new THREE.CylinderGeometry(radius, radius, thickness, 32),
      new THREE.MeshStandardMaterial({ color: COL_HATCH, metalness: 0.6, roughness: 0.4 })
    );
    disc.rotation.z = Math.PI / 2;
    g.add(disc);
    return g;
  }

  _initLockIndicators() {
    const geo = new THREE.SphereGeometry(0.15, 8, 8);

    // Inner lock indicator
    this._innerLockLamp = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color: COL_LOCK_LOCKED }));
    this._innerLockLamp.position.set(-2.0, 3.2, 0.2);
    this._airlockGroup.add(this._innerLockLamp);

    // Outer lock indicator
    this._outerLockLamp = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color: COL_LOCK_LOCKED }));
    this._outerLockLamp.position.set(2.0, 3.2, 0.2);
    this._airlockGroup.add(this._outerLockLamp);
  }

  _initPipingAndValves() {
    this._eqPipeMat = new THREE.MeshStandardMaterial({ color: COL_PIPE, metalness: 0.6, roughness: 0.4 });
    this._ventPipeMat = new THREE.MeshStandardMaterial({ color: COL_PIPE, metalness: 0.6, roughness: 0.4 });
    const anchor = this._airlockGroup.position;

    // EQ valve
    this._eqValve = this._createValveAssembly();
    this._eqValve.position.set(anchor.x - 2.75, anchor.y + 1.5, anchor.z);
    this._root.add(this._eqValve);

    // Vent valve
    this._ventValve = this._createValveAssembly();
    this._ventValve.position.set(anchor.x + 2.75, anchor.y + 1.5, anchor.z);
    this._root.add(this._ventValve);
  }

  _createValveAssembly() {
    const g = new THREE.Group();
    const body = new THREE.Mesh(new THREE.CylinderGeometry(0.14, 0.14, 0.22, 16), new THREE.MeshStandardMaterial({ color: COL_VALVE_BODY }));
    g.add(body);
    const wheel = new THREE.Mesh(new THREE.TorusGeometry(0.11, 0.02, 6, 16), new THREE.MeshStandardMaterial({ color: COL_HANDWHEEL }));
    wheel.position.y = 0.38;
    wheel.rotation.x = Math.PI / 2;
    g.add(wheel);
    g.userData.wheel = wheel;
    return g;
  }

  _initVacuumPump() {
    this._pumpGroup = new THREE.Group();
    const anchor = this._airlockGroup.position;
    this._pumpGroup.position.set(anchor.x, anchor.y + 2.0, anchor.z);

    this._pumpHousingMat = new THREE.MeshStandardMaterial({ color: COL_PUMP_HOUSING });
    this._pumpGroup.add(new THREE.Mesh(new THREE.CylinderGeometry(0.35, 0.35, 0.5, 24), this._pumpHousingMat));

    this._impeller = new THREE.Group();
    const bladeMat = new THREE.MeshStandardMaterial({ color: 0x8aaa5a });
    for (let i = 0; i < 5; i++) {
      const blade = new THREE.Mesh(new THREE.PlaneGeometry(0.28, 0.06), bladeMat);
      blade.rotation.z = (Math.PI * 2 / 5) * i;
      this._impeller.add(blade);
    }
    this._impeller.position.y = 0.38;
    this._impeller.rotation.x = -Math.PI / 2;
    this._pumpGroup.add(this._impeller);

    const glowMat = new THREE.SpriteMaterial({ color: 0x88cc44, transparent: true, opacity: 0, blending: THREE.AdditiveBlending });
    this._pumpGlow = new THREE.Sprite(glowMat);
    this._pumpGlow.scale.set(1.4, 1.4, 1);
    this._pumpGroup.add(this._pumpGlow);

    this._root.add(this._pumpGroup);
  }

  // =========================================================================
  // Particles & Animations
  // =========================================================================
  _initParticleSystems() {
    this._ventPS = this._createParticlePool(200, 0xccddaa, 0.1);
    this._eqPS = this._createParticlePool(150, 0xaaccee, 0.1);
    this._root.add(this._ventPS.points);
    this._root.add(this._eqPS.points);
  }

  _createParticlePool(count, color, size) {
    const positions = new Float32Array(count * 3);
    const velocities = new Float32Array(count * 3);
    const ages = new Float32Array(count);
    const lifetimes = new Float32Array(count);
    const active = new Uint8Array(count);

    for (let i = 0; i < count; i++) { positions[i * 3 + 1] = -100; }

    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    const mat = new THREE.PointsMaterial({ color, size, transparent: true, opacity: 0.6, blending: THREE.AdditiveBlending });
    return { points: new THREE.Points(geo, mat), positions, velocities, ages, lifetimes, active, count };
  }

  _emitParticle(pool, origin, velocity, lifetime) {
    for (let i = 0; i < pool.count; i++) {
      if (pool.active[i] === 0) {
        pool.positions[i * 3] = origin.x; pool.positions[i * 3 + 1] = origin.y; pool.positions[i * 3 + 2] = origin.z;
        pool.velocities[i * 3] = velocity.x; pool.velocities[i * 3 + 1] = velocity.y; pool.velocities[i * 3 + 2] = velocity.z;
        pool.ages[i] = 0; pool.lifetimes[i] = lifetime; pool.active[i] = 1;
        return;
      }
    }
  }

  _updateParticlePool(pool, dt) {
    const pos = pool.positions;
    for (let i = 0; i < pool.count; i++) {
      if (pool.active[i] === 0) continue;
      pool.ages[i] += dt;
      if (pool.ages[i] >= pool.lifetimes[i]) { pool.active[i] = 0; pos[i * 3 + 1] = -100; continue; }
      pos[i * 3] += pool.velocities[i * 3] * dt;
      pos[i * 3 + 1] += pool.velocities[i * 3 + 1] * dt;
      pos[i * 3 + 2] += pool.velocities[i * 3 + 2] * dt;
    }
    pool.points.geometry.attributes.position.needsUpdate = true;
  }

  _initLabels() {
    this._labels = {
      base: this._createTextSprite("UNDERHILL HABITAT", 0, 18, -25),
      airlock: this._createTextSprite("AIRLOCK SYSTEMS", 0, 4, -4),
    };
    Object.values(this._labels).forEach(s => this._root.add(s));

    // dynamic state label above airlock
    this._stateLabel = this._createTextSprite("", 0, 6, -4);
    this._root.add(this._stateLabel);

    // pool for transient messages
    this._flashMessages = [];
  }

  _createTextSprite(text, x, y, z) {
    const canvas = document.createElement("canvas");
    canvas.width = 512; canvas.height = 128;
    const ctx = canvas.getContext("2d");
    ctx.fillStyle = "rgba(0,0,0,0.5)"; ctx.fillRect(0, 0, 512, 128);
    ctx.fillStyle = "#ffffff"; ctx.font = "bold 40px monospace"; ctx.textAlign = "center";
    ctx.fillText(text, 256, 75);
    const texture = new THREE.CanvasTexture(canvas);
    const sprite = new THREE.Sprite(new THREE.SpriteMaterial({ map: texture }));
    sprite.position.set(x, y, z);
    sprite.scale.set(10, 2.5, 1);
    return sprite;
  }

  _updateTextSprite(sprite, text) {
    const canvas = sprite.material.map.image;
    const ctx = canvas.getContext("2d");
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = "rgba(0,0,0,0.5)"; ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = "#ffffff"; ctx.font = "bold 40px monospace"; ctx.textAlign = "center";
    ctx.fillText(text, canvas.width/2, 75);
    sprite.material.map.needsUpdate = true;
  }

  flashMessage(text, duration = 3.0) {
    const msgSprite = this._createTextSprite(text, 0, 6, -4);
    msgSprite.material.transparent = true;
    msgSprite.userData = { age: 0, duration };
    this._root.add(msgSprite);
    this._flashMessages.push(msgSprite);
  }

  _startAnimationLoop() {
    const clock = new THREE.Clock();
    const animate = () => {
      requestAnimationFrame(animate);
      const dt = clock.getDelta();
      this.controls.update();
      this._updateDust(dt);

      const s = this._lastSnapshot;
      if (s) {
        // Update Airlock
        this._innerPivot.rotation.z = (s.inner_door_position_pct / 100) * (Math.PI / 2);
        this._outerPivot.rotation.z = (s.outer_door_position_pct / 100) * (-Math.PI / 2);

        const pressureRatio = Math.min(1, s.pressure_pa / HABITAT_PRESSURE_PA);
        this._pressureFogMat.opacity = pressureRatio * 0.2;

        if (s.pump_on) {
          this._impeller.rotation.z += 10 * dt;
          this._pumpGlow.material.opacity = 0.5 + Math.sin(Date.now() * 0.01) * 0.2;
        } else {
          this._pumpGlow.material.opacity = 0;
        }

        // Door Locks
        this._innerLockLamp.material.color.set(s.inner_lock_engaged ? COL_LOCK_LOCKED : COL_LOCK_UNLOCKED);
        this._outerLockLamp.material.color.set(s.outer_lock_engaged ? COL_LOCK_LOCKED : COL_LOCK_UNLOCKED);

        // Subsystem Pod Glows
        const alarmPulse = 0.5 + Math.sin(Date.now() * 0.01) * 0.5;
        if (s.alarms.out_of_spec) {
          this._chamberMat.emissive.setRGB(alarmPulse, 0, 0);
          this._chamberMat.emissiveIntensity = 1.0;
        } else {
          this._chamberMat.emissiveIntensity = 0;
        }

        // ECLSS indicator (using pressure as proxy for "active")
        if (this._eclssPod?.userData?.indicatorCap?.material) {
          if (s.eclss_pressure_pa > 50000) {
            this._eclssPod.userData.indicatorCap.material.opacity =
              0.4 + Math.sin(Date.now() * 0.002) * 0.2;
          } else {
            this._eclssPod.userData.indicatorCap.material.opacity = 0.1;
          }
        }

        // Sabatier indicator (using temp as proxy)
        if (this._sabatierPod?.userData?.indicatorCap?.material) {
          if (s.sabatier_reactor_temp_c > 50) {
            this._sabatierPod.userData.indicatorCap.material.opacity =
              0.4 + Math.sin(Date.now() * 0.003) * 0.2;
          } else {
            this._sabatierPod.userData.indicatorCap.material.opacity = 0.1;
          }
        }

        // update state label
        if (s.state_name) {
          this._updateTextSprite(this._stateLabel, s.state_name.toUpperCase());
        }

        // Emit particles
        const airlockPos = this._airlockGroup.position;
        if (s.vent_valve_pct > 1) {
          this._emitParticle(
            this._ventPS,
            { x: airlockPos.x + 2.75, y: airlockPos.y + 1.5, z: airlockPos.z },
            { x: 5, y: 1, z: 0 },
            1.0,
          );
        }
        if (s.equalize_valve_pct > 1) {
          this._emitParticle(
            this._eqPS,
            { x: airlockPos.x - 2.75, y: airlockPos.y + 1.5, z: airlockPos.z },
            { x: -5, y: 1, z: 0 },
            1.0,
          );
        }
      }

      // update flash messages
      this._flashMessages = this._flashMessages.filter(sprite => {
        sprite.userData.age += dt;
        const t = sprite.userData.age / sprite.userData.duration;
        sprite.material.opacity = 1.0 - t;
        if (sprite.userData.age >= sprite.userData.duration) {
          this._root.remove(sprite);
          return false;
        }
        return true;
      });

      this._updateParticlePool(this._ventPS, dt);
      this._updateParticlePool(this._eqPS, dt);

      this.renderer.render(this.scene, this.camera);
    };
    animate();
  }
}
