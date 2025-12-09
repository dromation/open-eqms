from flask import Flask, render_template, request, jsonify
from eqms.core import app_launcher

app = Flask(__name__)

@app.route('/')
def index():
    return render_template('index.html')

@app.route('/launch/<app_name>', methods=['POST'])
def launch_app(app_name):
    response = app_launcher.launch(app_name)
    return jsonify(response)

if __name__ == '__main__':
    app.run(debug=True)