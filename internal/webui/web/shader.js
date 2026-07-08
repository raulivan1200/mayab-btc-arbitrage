// Shader premium interactivo con Three.js
// Implementado para darle un look "mamalón" a la UI (Cyber/Tech)

const initShader = () => {
  const canvas = document.getElementById('bg-shader');
  if (!canvas) return;

  const scene = new THREE.Scene();
  const camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  
  const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));

  // Shader Uniforms
  const uniforms = {
    u_time: { value: 0.0 },
    u_resolution: { value: new THREE.Vector2(window.innerWidth, window.innerHeight) },
    u_mouse: { value: new THREE.Vector2() }
  };

  // Vertex Shader
  const vertexShader = `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = vec4(position, 1.0);
    }
  `;

  // Fragment Shader (Matrix / Wave effect)
  const fragmentShader = `
    uniform float u_time;
    uniform vec2 u_resolution;
    varying vec2 vUv;

    // Helper functions
    float random(vec2 st) {
        return fract(sin(dot(st.xy, vec2(12.9898,78.233))) * 43758.5453123);
    }

    void main() {
      vec2 st = gl_FragCoord.xy / u_resolution.xy;
      st.x *= u_resolution.x / u_resolution.y;

      // Base color (theme background)
      vec3 color = vec3(0.04, 0.05, 0.04);
      
      // Moving grid / matrix lines
      vec2 grid = fract(st * 15.0 - u_time * 0.2);
      float line = smoothstep(0.0, 0.05, grid.x) * smoothstep(0.0, 0.05, grid.y);
      color += vec3(0.02, 0.1, 0.05) * (1.0 - line);

      // Cyber wave effect
      float wave = sin(st.x * 5.0 + u_time * 2.0) * 0.5 + 0.5;
      float distance = abs(st.y - wave * 0.2 - 0.4);
      float glow = 0.01 / distance;
      
      // Add primary accent color (Mayab Green / Cyan)
      color += vec3(0.12, 0.90, 0.60) * glow * 0.3;

      // Dynamic tech particles
      float particle = smoothstep(0.98, 1.0, random(floor(st * 30.0 + u_time)));
      color += vec3(1.0, 0.7, 0.1) * particle * 0.5;

      gl_FragColor = vec4(color, 0.75); // Adjust alpha for translucency
    }
  `;

  const material = new THREE.ShaderMaterial({
    vertexShader,
    fragmentShader,
    uniforms,
    transparent: true
  });

  const geometry = new THREE.PlaneGeometry(2, 2);
  const mesh = new THREE.Mesh(geometry, material);
  scene.add(mesh);

  // Resize handler
  window.addEventListener('resize', () => {
    renderer.setSize(window.innerWidth, window.innerHeight);
    uniforms.u_resolution.value.set(window.innerWidth, window.innerHeight);
  });

  // Mouse interactivity
  window.addEventListener('mousemove', (e) => {
    uniforms.u_mouse.value.set(
      e.clientX / window.innerWidth,
      1.0 - e.clientY / window.innerHeight
    );
  });

  // Animation loop
  const animate = () => {
    requestAnimationFrame(animate);
    uniforms.u_time.value += 0.01;
    renderer.render(scene, camera);
  };

  animate();
};

window.addEventListener('DOMContentLoaded', () => {
  if (typeof THREE !== 'undefined') {
    initShader();
  }
});
