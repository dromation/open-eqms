function toggleMenu() {
    // Toggle the visibility of the submenu
    const submenu = document.querySelector('.submenu');
    submenu.classList.toggle('active');
}

function showCommands() {
    // Show commands menu or perform related actions
    alert('Commands menu clicked');
}

function showSettings() {
    // Show settings menu or perform related actions
    alert('Settings menu clicked');
}

function showInfo() {
    // Show information or perform related actions
    alert('Info menu clicked');
}

async function launchApp(appName) {
    // POST to backend launch endpoint; fall back to alert on failure.
    try {
        const res = await fetch(`/launch/${encodeURIComponent(appName)}`, {
            method: 'POST'
        });
        if (!res.ok) {
            throw new Error(`HTTP ${res.status}`);
        }
        const data = await res.json();
        alert(data.message || `Launch requested for ${appName}`);
    } catch (err) {
        console.error(err);
        alert(`Failed to launch ${appName}`);
    }
}
