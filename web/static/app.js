// WASM module loader with fallbacks
let wasmModule = null;
let app = null;
let wasmSupported = false;

// Check for WASM support
async function checkWasmSupport() {
    try {
        if (typeof WebAssembly === 'object') {
            const result = await WebAssembly.validate(new Uint8Array([0, 97, 115, 109, 1, 0, 0, 0]));
            wasmSupported = result;
            console.log('WASM support:', wasmSupported);
        }
    } catch (e) {
        console.log('WASM not supported:', e);
        wasmSupported = false;
    }
    return wasmSupported;
}

// Load WASM module
async function loadWasm() {
    if (!wasmSupported) {
        console.log('Using JavaScript fallback (WASM not available)');
        return null;
    }

    try {
        // Try to load WASM module
        const wasm = await import('/pkg/buddy_schedule_web.js');
        if (wasm.default) {
            await wasm.default();
        }
        wasmModule = wasm;
        console.log('WASM module loaded successfully');
        return wasm;
    } catch (e) {
        console.warn('Failed to load WASM (this is OK if not built yet):', e.message);
        console.log('Using JavaScript fallback');
        return null;
    }
}

// Initialize renderer (WebGL or Canvas 2D)
async function initRenderer() {
    const canvas = document.getElementById('render-canvas');
    if (!canvas) return;

    // Resize canvas
    function resizeCanvas() {
        const content = document.getElementById('content');
        canvas.width = content.clientWidth;
        canvas.height = content.clientHeight;
    }
    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);

    if (wasmModule && wasmModule.App) {
        try {
            app = new wasmModule.App('render-canvas');
            await app.init();
            await app.render();
            console.log('WASM renderer initialized');
        } catch (e) {
            console.error('WASM renderer failed:', e);
            initCanvas2DFallback();
        }
    } else {
        initCanvas2DFallback();
    }
}

function initCanvas2DFallback() {
    const canvas = document.getElementById('render-canvas');
    const ctx = canvas.getContext('2d');
    
    function draw() {
        // Clear
        ctx.fillStyle = '#1a1a26';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        // Draw welcome text
        ctx.fillStyle = '#4a9eff';
        ctx.font = '32px sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText('Buddy Schedule', canvas.width / 2, canvas.height / 2);
        
        // Draw subtitle
        ctx.fillStyle = '#a0a0a0';
        ctx.font = '16px sans-serif';
        ctx.fillText('Schedule Management System', canvas.width / 2, canvas.height / 2 + 40);
    }
    
    draw();
    window.addEventListener('resize', () => {
        const content = document.getElementById('content');
        canvas.width = content.clientWidth;
        canvas.height = content.clientHeight;
        draw();
    });
}

// API calls with fallback
async function apiCall(path, method = 'GET', body = null) {
    const url = `/api${path}`;
    const options = {
        method,
        headers: {
            'Content-Type': 'application/json',
        },
    };

    // Add auth token
    const token = wasmModule ? wasmModule.get_auth_token() : localStorage.getItem('auth_token');
    if (token) {
        options.headers['Authorization'] = `Bearer ${token}`;
    }

    if (body) {
        options.body = JSON.stringify(body);
    }

    try {
        const response = await fetch(url, options);
        const data = await response.json();
        
        if (!response.ok) {
            throw new Error(data.error || 'Request failed');
        }
        
        return data;
    } catch (error) {
        console.error('API call failed:', error);
        throw error;
    }
}

// Auth functions
function setAuthToken(token) {
    if (wasmModule) {
        wasmModule.set_auth_token(token);
    } else {
        localStorage.setItem('auth_token', token);
    }
}

function getAuthToken() {
    if (wasmModule) {
        return wasmModule.get_auth_token();
    }
    return localStorage.getItem('auth_token') || '';
}

function clearAuthToken() {
    if (wasmModule) {
        wasmModule.clear_auth_token();
    } else {
        localStorage.removeItem('auth_token');
    }
}

// UI State
let currentUser = null;
let schedules = [];

// Check auth status
function checkAuth() {
    const token = getAuthToken();
    if (token) {
        loadUser();
        showMainScreen();
    } else {
        showLoginScreen();
    }
}

async function loadUser() {
    try {
        const user = await apiCall('/me');
        currentUser = user;
        document.getElementById('user-email').textContent = user.email;
        document.getElementById('logout-btn').style.display = 'block';
        loadSchedules();
    } catch (error) {
        console.error('Failed to load user:', error);
        clearAuthToken();
        showLoginScreen();
    }
}

async function loadSchedules() {
    try {
        schedules = await apiCall('/schedules');
        renderSchedules();
    } catch (error) {
        showError('Failed to load schedules: ' + error.message);
    }
}

function renderSchedules() {
    const list = document.getElementById('schedule-list');
    list.innerHTML = '';
    
    schedules.forEach(schedule => {
        const li = document.createElement('li');
        li.textContent = schedule.schedule.name;
        li.onclick = () => showSchedule(schedule.schedule.id);
        list.appendChild(li);
    });
}

function showSchedule(scheduleId) {
    const schedule = schedules.find(s => s.schedule.id === scheduleId);
    if (!schedule) return;
    
    document.getElementById('schedule-name').textContent = schedule.schedule.name;
    document.getElementById('schedule-info').innerHTML = `
        <p><strong>Subject:</strong> ${schedule.schedule.subject_name} (${schedule.schedule.subject_type})</p>
        <p><strong>Role:</strong> ${schedule.role}</p>
    `;
    document.getElementById('schedule-view').style.display = 'block';
    document.getElementById('render-canvas').style.display = 'none';
}

function showLoginScreen() {
    document.getElementById('login-screen').style.display = 'flex';
    document.getElementById('register-screen').style.display = 'none';
    document.getElementById('main-screen').style.display = 'none';
    document.getElementById('logout-btn').style.display = 'none';
}

function showRegisterScreen() {
    document.getElementById('login-screen').style.display = 'none';
    document.getElementById('register-screen').style.display = 'flex';
    document.getElementById('main-screen').style.display = 'none';
}

function showMainScreen() {
    document.getElementById('login-screen').style.display = 'none';
    document.getElementById('register-screen').style.display = 'none';
    document.getElementById('main-screen').style.display = 'flex';
    document.getElementById('schedule-view').style.display = 'none';
    document.getElementById('render-canvas').style.display = 'block';
}

function showError(message) {
    const errorEl = document.getElementById('error-message');
    errorEl.textContent = message;
    errorEl.style.display = 'block';
    setTimeout(() => {
        errorEl.style.display = 'none';
    }, 5000);
}

// Event listeners
document.getElementById('login-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const email = document.getElementById('login-email').value;
    const password = document.getElementById('login-password').value;
    
    try {
        const response = await apiCall('/auth/login', 'POST', { email, password });
        setAuthToken(response.token);
        await loadUser();
        showMainScreen();
    } catch (error) {
        showError('Login failed: ' + error.message);
    }
});

document.getElementById('register-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const email = document.getElementById('register-email').value;
    const password = document.getElementById('register-password').value;
    
    try {
        const response = await apiCall('/auth/register', 'POST', { email, password });
        setAuthToken(response.token);
        await loadUser();
        showMainScreen();
    } catch (error) {
        showError('Registration failed: ' + error.message);
    }
});

document.getElementById('show-register').addEventListener('click', (e) => {
    e.preventDefault();
    showRegisterScreen();
});

document.getElementById('show-login').addEventListener('click', (e) => {
    e.preventDefault();
    showLoginScreen();
});

document.getElementById('logout-btn').addEventListener('click', () => {
    clearAuthToken();
    currentUser = null;
    schedules = [];
    showLoginScreen();
});

// Initialize
(async () => {
    await checkWasmSupport();
    await loadWasm();
    await initRenderer();
    checkAuth();
})();
