<!-- ONLY USE HTML IN THIS FILE, IT GETS INCLUDED IN THE WebGUI -->
<section class="section-container mt-0 mb-4" aria-labelledby="platform-support-title">
    <h2 id="platform-support-title" class="section-title px-4 pt-4">🖥️ Platform Support</h2>
    <div class="table-wrapper" tabindex="0">
        <table class="info-table">
            <thead id="platform-support-table-header">
                <tr>
                    <th scope="col">Component</th>
                    <th scope="col">Linux</th>
                    <th scope="col">macOS</th>
                    <th scope="col">Windows</th>
                </tr>
            </thead>
            <tbody>
                <tr>
                    <td>Web GUI</td>
                    <td>✅ (any modern browser)</td>
                    <td>✅ (any modern browser)</td>
                    <td>✅ (any modern browser)</td>
                </tr>
                <tr>
                    <td>Coordinator</td>
                    <td>✅ Binary<br>✅ Docker</td>
                    <td>✅ Binary<br>❌ Docker<br>✅ Linux VM<br>(bridged networking)</td>
                    <td>❌ Binary<br>❌ Docker<br>❌ WSL<br>✅ Linux VM<br>(bridged networking)</td>
                </tr>
                <tr>
                    <td>Host Agent</td>
                    <td>✅ Binary</td>
                    <td>✅ Binary</td>
                    <td>✅ Self-extracting<br>(manual service setup required)</td>
                </tr>
                <tr>
                    <td>Client<br>(Script)</td>
                    <td>✅ Shell<br>✅ Docker</td>
                    <td>✅ Shell<br>✅ Docker</td>
                    <td>✅ PowerShell<br>✅ Docker<br>✅ WSL (Shell)</td>
                </tr>
                <tr>
                    <td>Direct Control<br>(Script)</td>
                    <td>✅ Shell</td>
                    <td>✅ Shell</td>
                    <td>✅ PowerShell</td>
                </tr>
            </tbody>
        </table>
    </div>
</section>
