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

function launchApp(appName) {
    // Logic to launch the specific app
    alert(`Launching ${appName} app`);
}
