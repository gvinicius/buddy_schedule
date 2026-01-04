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
        
        // Handle 204 No Content (empty response) - return early
        if (response.status === 204) {
            if (!response.ok) {
                throw new Error('Request failed');
            }
            return null;
        }
        
        // Check if response has content
        const contentType = response.headers.get('content-type');
        const hasJsonContent = contentType && contentType.includes('application/json');
        
        // For responses without JSON content type or empty body, return null if OK
        if (!hasJsonContent) {
            if (!response.ok) {
                const text = await response.text().catch(() => 'Request failed');
                throw new Error(text || 'Request failed');
            }
            return null;
        }
        
        // Try to parse JSON
        const text = await response.text();
        if (!text || text.trim() === '') {
            if (!response.ok) {
                throw new Error('Request failed');
            }
            return null;
        }
        
        let data;
        try {
            data = JSON.parse(text);
        } catch (e) {
            // If parsing fails but response is OK, return null
            if (response.ok) {
                return null;
            }
            // If parsing fails and response is not OK, throw error
            throw new Error(text || 'Request failed');
        }
        
        if (!response.ok) {
            throw new Error(data.error || data.message || 'Request failed');
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
async function checkAuth() {
    const token = getAuthToken();
    if (token) {
        // Try to load user first, then show main screen
        try {
            await loadUser();
        showMainScreen();
        } catch (error) {
            // Token is invalid, clear it and show login
            clearAuthToken();
            showLoginScreen();
        }
    } else {
        showLoginScreen();
    }
}

async function loadUser() {
    const user = await apiCall('/me');
    currentUser = user;
    document.getElementById('user-email').textContent = user.email;
    document.getElementById('logout-btn').style.display = 'block';
    await loadSchedules();
    
    // After loading schedules, check URL for schedule ID
    const scheduleId = getScheduleFromUrl();
    if (scheduleId) {
        // Convert to string for comparison (UUIDs might be compared as strings)
        const schedule = schedules.find(s => String(s.schedule.id) === String(scheduleId));
        if (schedule) {
            console.log('Loading schedule from URL:', scheduleId);
            await showSchedule(scheduleId);
        } else {
            console.warn('Schedule not found in loaded schedules. URL scheduleId:', scheduleId, 'Available:', schedules.map(s => s.schedule.id));
        }
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
    
    // Get current schedule from URL
    const currentScheduleId = getScheduleFromUrl();
    
    schedules.forEach(schedule => {
        const li = document.createElement('li');
        li.textContent = schedule.schedule.name;
        li.dataset.scheduleId = schedule.schedule.id;
        // Convert to string for comparison (UUIDs might be compared as strings)
        if (String(schedule.schedule.id) === String(currentScheduleId)) {
            li.classList.add('active');
        }
        li.onclick = () => {
            showSchedule(schedule.schedule.id);
            updateUrlForSchedule(schedule.schedule.id);
        };
        list.appendChild(li);
    });
}

function getScheduleFromUrl() {
    const hash = window.location.hash;
    if (hash && hash.startsWith('#schedule-')) {
        return hash.replace('#schedule-', '');
    }
    return null;
}

function updateUrlForSchedule(scheduleId) {
    if (scheduleId) {
        window.location.hash = `schedule-${scheduleId}`;
    } else {
        window.location.hash = '';
    }
}

let currentScheduleId = null;
let currentWeekStart = null;
let shifts = [];
let scheduleMembers = [];
let editingShift = null;
let editingDayIndex = null;
let editingPeriod = null;

function getWeekStart(date = new Date()) {
    const d = new Date(date);
    const day = d.getDay();
    const diff = d.getDate() - day + (day === 0 ? -6 : 1); // Adjust to Monday
    return new Date(d.setDate(diff));
}

function formatDate(date) {
    return date.toISOString().split('T')[0];
}

function formatWeekRange(start) {
    const end = new Date(start);
    end.setDate(end.getDate() + 6);
    const startStr = start.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    const endStr = end.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
    return `${startStr} - ${endStr}`;
}

async function loadShifts(scheduleId, weekStart) {
    try {
        const from = new Date(weekStart);
        const to = new Date(weekStart);
        to.setDate(to.getDate() + 7);
        
        const fromStr = from.toISOString();
        const toStr = to.toISOString();
        
        shifts = await apiCall(`/schedules/${scheduleId}/shifts?from=${encodeURIComponent(fromStr)}&to=${encodeURIComponent(toStr)}`);
        renderCalendar();
    } catch (error) {
        console.error('Failed to load shifts:', error);
        shifts = [];
        renderCalendar();
    }
}

async function loadScheduleMembers(scheduleId) {
    try {
        const members = await apiCall(`/schedules/${scheduleId}/members`);
        scheduleMembers = members.map(m => m.user);
    } catch (error) {
        console.error('Failed to load members:', error);
        scheduleMembers = currentUser ? [currentUser] : [];
    }
}

function generateGoogleCalendarLink(shift, scheduleName) {
    const start = new Date(shift.starts_at);
    const end = new Date(shift.ends_at);
    
    // Format dates for Google Calendar (YYYYMMDDTHHMMSSZ)
    const formatGoogleDate = (date) => {
        const year = date.getUTCFullYear();
        const month = String(date.getUTCMonth() + 1).padStart(2, '0');
        const day = String(date.getUTCDate()).padStart(2, '0');
        const hours = String(date.getUTCHours()).padStart(2, '0');
        const minutes = String(date.getUTCMinutes()).padStart(2, '0');
        const seconds = String(date.getUTCSeconds()).padStart(2, '0');
        return `${year}${month}${day}T${hours}${minutes}${seconds}Z`;
    };
    
    const startStr = formatGoogleDate(start);
    const endStr = formatGoogleDate(end);
    
    // Google Calendar URL format
    const params = new URLSearchParams({
        action: 'TEMPLATE',
        text: `${scheduleName} - ${shift.period}`,
        dates: `${startStr}/${endStr}`,
        details: `Period: ${shift.period}\nSchedule: ${scheduleName}`,
    });
    
    return `https://calendar.google.com/calendar/render?${params.toString()}`;
}

function openShiftEditModal(dayIndex, period, existingShift) {
    editingDayIndex = dayIndex;
    editingPeriod = period;
    editingShift = existingShift;
    
    const modal = document.getElementById('edit-shift-modal');
    const form = document.getElementById('edit-shift-form');
    const startTimeInput = document.getElementById('shift-start-time');
    const endTimeInput = document.getElementById('shift-end-time');
    const userSelect = document.getElementById('shift-user-select');
    const notesInput = document.getElementById('shift-notes');
    const googleLink = document.getElementById('google-calendar-link');
    const deleteBtn = document.getElementById('delete-shift-btn');
    
    // Populate user dropdown
    userSelect.innerHTML = '<option value="">Unassigned</option>';
    scheduleMembers.forEach(member => {
        const option = document.createElement('option');
        option.value = member.id;
        option.textContent = member.email;
        if (existingShift && existingShift.assigned_user_id === member.id) {
            option.selected = true;
        }
        userSelect.appendChild(option);
    });
    
    if (existingShift) {
        // Edit existing shift
        const start = new Date(existingShift.starts_at);
        const end = new Date(existingShift.ends_at);
        startTimeInput.value = `${String(start.getHours()).padStart(2, '0')}:${String(start.getMinutes()).padStart(2, '0')}`;
        endTimeInput.value = `${String(end.getHours()).padStart(2, '0')}:${String(end.getMinutes()).padStart(2, '0')}`;
        deleteBtn.style.display = 'block';
        
        // Generate Google Calendar link
        const scheduleName = document.getElementById('schedule-name').textContent;
        googleLink.href = generateGoogleCalendarLink(existingShift, scheduleName);
        googleLink.style.display = 'block';
    } else {
        // New shift - use default times for period
        const periodTimes = getPeriodTimes(period);
        startTimeInput.value = periodTimes.start;
        endTimeInput.value = periodTimes.end;
        deleteBtn.style.display = 'none';
        googleLink.style.display = 'none';
    }
    
    notesInput.value = '';
    modal.style.display = 'flex';
    modal.classList.add('show');
    startTimeInput.focus();
}

function closeShiftEditModal() {
    const modal = document.getElementById('edit-shift-modal');
    modal.style.display = 'none';
    modal.classList.remove('show');
    editingShift = null;
    editingDayIndex = null;
    editingPeriod = null;
}

function getPeriodTimes(period) {
    const times = {
        morning: { start: '08:00', end: '12:00' },
        afternoon: { start: '12:00', end: '18:00' },
        night: { start: '18:00', end: '22:00' },
        sleep: { start: '22:00', end: '08:00' }
    };
    return times[period] || { start: '00:00', end: '00:00' };
}


function renderCalendar() {
    const grid = document.getElementById('calendar-grid');
    if (!grid) return;
    
    grid.innerHTML = '';
    
    // Create header with days - first cell is empty for period label column
    const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
    const header = document.createElement('div');
    header.className = 'calendar-header-row';
    // Empty cell for period label column
    const emptyHeader = document.createElement('div');
    emptyHeader.className = 'calendar-period-label';
    header.appendChild(emptyHeader);
    // Day headers
    days.forEach((day, idx) => {
        const dayDate = new Date(currentWeekStart);
        dayDate.setDate(dayDate.getDate() + idx);
        const dayCell = document.createElement('div');
        dayCell.className = 'calendar-day-header';
        dayCell.innerHTML = `
            <div class="day-name">${day}</div>
            <div class="day-date">${dayDate.getDate()}</div>
        `;
        header.appendChild(dayCell);
    });
    grid.appendChild(header);
    
    // Create time slots for each day
    const periods = ['morning', 'afternoon', 'night', 'sleep'];
    const periodLabels = { morning: 'Morning', afternoon: 'Afternoon', night: 'Night', sleep: 'Sleep' };
    
    periods.forEach(period => {
        const row = document.createElement('div');
        row.className = 'calendar-period-row';
        
        // Period label
        const periodLabel = document.createElement('div');
        periodLabel.className = 'calendar-period-label';
        periodLabel.textContent = periodLabels[period];
        row.appendChild(periodLabel);
        
        // Day cells
        for (let i = 0; i < 7; i++) {
            const dayDate = new Date(currentWeekStart);
            dayDate.setDate(dayDate.getDate() + i);
            dayDate.setHours(0, 0, 0, 0);
            
            const cell = document.createElement('div');
            cell.className = 'calendar-cell';
            cell.dataset.day = i;
            cell.dataset.period = period;
            cell.setAttribute('role', 'button');
            cell.setAttribute('tabindex', '0');
            cell.setAttribute('aria-label', `${days[i]} ${dayDate.getDate()} ${periodLabels[period]}`);
            
            // Find shifts for this day and period
            const dayShifts = shifts.filter(shift => {
                const shiftDate = new Date(shift.starts_at);
                shiftDate.setHours(0, 0, 0, 0);
                return shiftDate.getTime() === dayDate.getTime() && shift.period === period;
            });
            
            if (dayShifts.length > 0) {
                const shift = dayShifts[0];
                const startTime = new Date(shift.starts_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
                const endTime = new Date(shift.ends_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' });
                
                // Find assigned user name
                const assignedUser = scheduleMembers.find(m => m.id === shift.assigned_user_id);
                const userName = assignedUser ? assignedUser.email.split('@')[0] : '';
                
                cell.innerHTML = `
                    <div class="shift-time-display">${startTime} - ${endTime}</div>
                    ${userName ? `<div class="shift-user-display">${userName}</div>` : ''}
                `;
                cell.classList.add('has-shift');
                if (shift.assigned_user_id) {
                    cell.classList.add('assigned');
                }
                cell.dataset.shiftId = shift.id;
            } else {
                cell.innerHTML = '<div class="empty-cell-hint">Click to add...</div>';
                cell.classList.add('empty');
                cell.dataset.shiftId = '';
            }
            
            // Handle click to open edit modal
            cell.addEventListener('click', () => {
                openShiftEditModal(i, period, dayShifts[0] || null);
            });
            
            // Handle keyboard
            cell.addEventListener('keydown', (e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    openShiftEditModal(i, period, dayShifts[0] || null);
                }
            });
            
            row.appendChild(cell);
        }
        
        grid.appendChild(row);
    });
}

async function showSchedule(scheduleId) {
    // Convert to string for comparison (UUIDs might be compared as strings)
    const schedule = schedules.find(s => String(s.schedule.id) === String(scheduleId));
    if (!schedule) {
        console.warn('Schedule not found:', scheduleId, 'Available schedules:', schedules.map(s => s.schedule.id));
        return;
    }
    
    currentScheduleId = scheduleId;
    currentWeekStart = getWeekStart();
    
    document.getElementById('schedule-name').textContent = schedule.schedule.name;
    document.getElementById('schedule-info').innerHTML = `
        <p><strong>Subject:</strong> ${schedule.schedule.subject_name} (${schedule.schedule.subject_type})</p>
        <p><strong>Role:</strong> ${schedule.role}</p>
    `;
    document.getElementById('schedule-view').style.display = 'block';
    document.getElementById('render-canvas').style.display = 'none';
    
    // Update week display
    document.getElementById('current-week').textContent = formatWeekRange(currentWeekStart);
    
    // Load members and shifts
    await loadScheduleMembers(scheduleId);
    await loadShifts(scheduleId, currentWeekStart);
    
    // Update active state in list
    const listItems = document.querySelectorAll('#schedule-list li');
    listItems.forEach(li => {
        li.classList.remove('active');
        // Convert to string for comparison (UUIDs might be compared as strings)
        if (String(li.dataset.scheduleId) === String(scheduleId)) {
            li.classList.add('active');
        }
    });
}

function showLoginScreen() {
    document.getElementById('login-screen').style.display = 'flex';
    document.getElementById('register-screen').style.display = 'none';
    document.getElementById('main-screen').style.display = 'none';
    document.getElementById('logout-btn').style.display = 'none';
    document.getElementById('user-email').textContent = '';
    currentUser = null;
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

function showSuccess(message) {
    const successEl = document.createElement('div');
    successEl.className = 'success';
    successEl.textContent = message;
    document.body.appendChild(successEl);
    setTimeout(() => {
        successEl.remove();
    }, 3000);
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

// Modal functions
function showNewScheduleModal() {
    const modal = document.getElementById('new-schedule-modal');
    modal.style.display = 'flex';
    modal.classList.add('show');
    document.getElementById('schedule-name-input').focus();
}

function hideNewScheduleModal() {
    const modal = document.getElementById('new-schedule-modal');
    modal.style.display = 'none';
    modal.classList.remove('show');
    document.getElementById('new-schedule-form').reset();
}

// New Schedule button handler
const newScheduleBtn = document.getElementById('new-schedule-btn');
if (newScheduleBtn) {
    newScheduleBtn.addEventListener('click', showNewScheduleModal);
}

// Close modal handlers
const closeModalBtn = document.getElementById('close-modal');
const cancelBtn = document.getElementById('cancel-schedule-btn');
const modal = document.getElementById('new-schedule-modal');

if (closeModalBtn) {
    closeModalBtn.addEventListener('click', hideNewScheduleModal);
}
if (cancelBtn) {
    cancelBtn.addEventListener('click', hideNewScheduleModal);
}
if (modal) {
    modal.addEventListener('click', (e) => {
        if (e.target.id === 'new-schedule-modal') {
            hideNewScheduleModal();
        }
    });
}

// New Schedule form handler
const newScheduleForm = document.getElementById('new-schedule-form');
if (newScheduleForm) {
    newScheduleForm.addEventListener('submit', async (e) => {
    e.preventDefault();
    
    const name = document.getElementById('schedule-name-input').value.trim();
    const subjectType = document.getElementById('subject-type-select').value;
    const subjectName = document.getElementById('subject-name-input').value.trim();
    
    if (!name || !subjectType || !subjectName) {
        showError('Please fill in all fields');
        return;
    }
    
    try {
        const schedule = await apiCall('/schedules', 'POST', {
            name,
            subject_type: subjectType,
            subject_name: subjectName
        });
        hideNewScheduleModal();
        await loadSchedules();
        // API returns Schedule directly, not wrapped
        showSchedule(schedule.id);
        updateUrlForSchedule(schedule.id);
    } catch (error) {
        showError('Failed to create schedule: ' + error.message);
    }
    });
}

// Theme toggle
function initTheme() {
    const savedTheme = localStorage.getItem('theme') || 'dark';
    document.documentElement.setAttribute('data-theme', savedTheme);
    updateThemeButton(savedTheme);
}

function toggleTheme(e) {
    if (e) e.preventDefault();
    const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
    const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', newTheme);
    localStorage.setItem('theme', newTheme);
    updateThemeButton(newTheme);
}

function updateThemeButton(theme) {
    const button = document.getElementById('theme-toggle');
    if (button) {
        button.textContent = theme === 'dark' ? '🌙' : '☀️';
        button.setAttribute('aria-label', `Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`);
    }
}

// Theme toggle - use event delegation to ensure it works
document.addEventListener('DOMContentLoaded', () => {
    const themeToggleBtn = document.getElementById('theme-toggle');
    if (themeToggleBtn) {
        themeToggleBtn.addEventListener('click', toggleTheme);
    }
});

// Also set up click handler immediately if button exists
const themeToggleBtn = document.getElementById('theme-toggle');
if (themeToggleBtn) {
    themeToggleBtn.addEventListener('click', toggleTheme);
}

// Initialize
(async () => {
    // Initialize theme
    initTheme();
    
    // Hide all screens initially to prevent blinking
    document.getElementById('login-screen').style.display = 'none';
    document.getElementById('register-screen').style.display = 'none';
    document.getElementById('main-screen').style.display = 'none';
    
    await checkWasmSupport();
    await loadWasm();
    await initRenderer();
    await checkAuth();
    
    // Calendar navigation - set up event delegation since buttons are in schedule-view
    document.addEventListener('click', (e) => {
        if (e.target.id === 'prev-week-btn' || e.target.closest('#prev-week-btn')) {
            if (currentScheduleId && currentWeekStart) {
                currentWeekStart.setDate(currentWeekStart.getDate() - 7);
                const weekEl = document.getElementById('current-week');
                if (weekEl) {
                    weekEl.textContent = formatWeekRange(currentWeekStart);
                }
                loadShifts(currentScheduleId, currentWeekStart);
            }
        } else if (e.target.id === 'next-week-btn' || e.target.closest('#next-week-btn')) {
            if (currentScheduleId && currentWeekStart) {
                currentWeekStart.setDate(currentWeekStart.getDate() + 7);
                const weekEl = document.getElementById('current-week');
                if (weekEl) {
                    weekEl.textContent = formatWeekRange(currentWeekStart);
                }
                loadShifts(currentScheduleId, currentWeekStart);
            }
        }
    });
    
    // Shift edit modal handlers
    const closeShiftModalBtn = document.getElementById('close-shift-modal');
    const cancelShiftBtn = document.getElementById('cancel-shift-btn');
    const shiftModal = document.getElementById('edit-shift-modal');
    
    if (closeShiftModalBtn) {
        closeShiftModalBtn.addEventListener('click', closeShiftEditModal);
    }
    if (cancelShiftBtn) {
        cancelShiftBtn.addEventListener('click', closeShiftEditModal);
    }
    if (shiftModal) {
        shiftModal.addEventListener('click', (e) => {
            if (e.target.id === 'edit-shift-modal') {
                closeShiftEditModal();
            }
        });
    }
    
    // Shift edit form submission
    const editShiftForm = document.getElementById('edit-shift-form');
    if (editShiftForm) {
        editShiftForm.addEventListener('submit', async (e) => {
            e.preventDefault();
            if (editingDayIndex === null || !editingPeriod || !currentScheduleId) return;
            
            const startTime = document.getElementById('shift-start-time').value;
            const endTime = document.getElementById('shift-end-time').value;
            const userId = document.getElementById('shift-user-select').value;
            const notes = document.getElementById('shift-notes').value.trim();
            
            const dayDate = new Date(currentWeekStart);
            dayDate.setDate(dayDate.getDate() + editingDayIndex);
            
            const [startHour, startMin] = startTime.split(':').map(Number);
            const [endHour, endMin] = endTime.split(':').map(Number);
            
            let startsAt = new Date(dayDate);
            startsAt.setHours(startHour, startMin, 0, 0);
            
            let endsAt = new Date(dayDate);
            if (endHour < startHour || (endHour === startHour && endMin < startMin)) {
                endsAt.setDate(endsAt.getDate() + 1);
            }
            endsAt.setHours(endHour, endMin, 0, 0);
            
            try {
                if (editingShift) {
                    // Update existing shift
                    // First update times if changed
                    if (new Date(editingShift.starts_at).getTime() !== startsAt.getTime() ||
                        new Date(editingShift.ends_at).getTime() !== endsAt.getTime()) {
                        // Note: API doesn't have update endpoint, so we'd need to delete and recreate
                        // For now, we'll just update assignment and add comment
                    }
                    
                    // Update assignment
                    const assignPayload = {};
                    if (userId && userId.trim() !== '') {
                        assignPayload.assigned_user_id = userId;
                    } else {
                        assignPayload.assigned_user_id = null;
                    }
                    await apiCall(`/shifts/${editingShift.id}/assign`, 'POST', assignPayload);
                    
                    // Add comment if provided
                    if (notes) {
                        await apiCall(`/shifts/${editingShift.id}/comments`, 'POST', { body: notes });
                    }
                } else {
                    // Create new shift
                    const newShift = await apiCall(`/schedules/${currentScheduleId}/shifts`, 'POST', {
                        starts_at: startsAt.toISOString(),
                        ends_at: endsAt.toISOString(),
                        period: editingPeriod
                    });
                    
                    // Assign user if selected
                    if (userId && userId.trim() !== '') {
                        await apiCall(`/shifts/${newShift.id}/assign`, 'POST', {
                            assigned_user_id: userId
                        });
                    }
                    
                    // Add comment if provided
                    if (notes) {
                        await apiCall(`/shifts/${newShift.id}/comments`, 'POST', { body: notes });
                    }
                }
                
                closeShiftEditModal();
                await loadShifts(currentScheduleId, currentWeekStart);
            } catch (error) {
                showError('Failed to save shift: ' + error.message);
            }
        });
    }
    
    // Delete shift button
    const deleteShiftBtn = document.getElementById('delete-shift-btn');
    if (deleteShiftBtn) {
        deleteShiftBtn.addEventListener('click', async () => {
            if (!editingShift) return;
            if (!confirm('Are you sure you want to delete this shift?')) return;
            
            try {
                // Note: API doesn't have delete endpoint, so we'll just unassign
                await apiCall(`/shifts/${editingShift.id}/assign`, 'POST', {
                    assigned_user_id: null
                });
                closeShiftEditModal();
                await loadShifts(currentScheduleId, currentWeekStart);
            } catch (error) {
                showError('Failed to delete shift: ' + error.message);
            }
        });
    }
    
    // Add member modal handlers
    const addMemberBtn = document.getElementById('add-member-btn');
    const closeMemberModalBtn = document.getElementById('close-member-modal');
    const cancelMemberBtn = document.getElementById('cancel-member-btn');
    const memberModal = document.getElementById('add-member-modal');
    
    if (addMemberBtn) {
        addMemberBtn.addEventListener('click', () => {
            memberModal.style.display = 'flex';
            memberModal.classList.add('show');
            document.getElementById('member-email-input').focus();
        });
    }
    
    if (closeMemberModalBtn) {
        closeMemberModalBtn.addEventListener('click', () => {
            memberModal.style.display = 'none';
            memberModal.classList.remove('show');
            document.getElementById('add-member-form').reset();
        });
    }
    
    if (cancelMemberBtn) {
        cancelMemberBtn.addEventListener('click', () => {
            memberModal.style.display = 'none';
            memberModal.classList.remove('show');
            document.getElementById('add-member-form').reset();
        });
    }
    
    if (memberModal) {
        memberModal.addEventListener('click', (e) => {
            if (e.target.id === 'add-member-modal') {
                memberModal.style.display = 'none';
                memberModal.classList.remove('show');
            }
        });
    }
    
    // Add member form submission
    const addMemberForm = document.getElementById('add-member-form');
    if (addMemberForm) {
        addMemberForm.addEventListener('submit', async (e) => {
            e.preventDefault();
            if (!currentScheduleId) return;
            
            const email = document.getElementById('member-email-input').value.trim();
            const role = document.getElementById('member-role-select').value;
            
            try {
                await apiCall(`/schedules/${currentScheduleId}/members`, 'POST', {
                    email: email,
                    role: role
                });
                
                // Reload members list
                await loadScheduleMembers(currentScheduleId);
                
                memberModal.style.display = 'none';
                memberModal.classList.remove('show');
                document.getElementById('add-member-form').reset();
                showSuccess('Member added successfully');
            } catch (error) {
                showError('Failed to add member: ' + error.message);
            }
        });
    }
    
    // Listen for hash changes to handle back/forward navigation
    window.addEventListener('hashchange', async () => {
        if (currentUser) {
            // Make sure schedules are loaded
            if (schedules.length === 0) {
                await loadSchedules();
            }
            const scheduleId = getScheduleFromUrl();
            if (scheduleId) {
                console.log('Hash changed, loading schedule:', scheduleId);
                await showSchedule(scheduleId);
            } else {
                document.getElementById('schedule-view').style.display = 'none';
                document.getElementById('render-canvas').style.display = 'block';
                // Remove active class from all items
                document.querySelectorAll('#schedule-list li').forEach(li => {
                    li.classList.remove('active');
                });
                currentScheduleId = null;
            }
        }
    });
})();
