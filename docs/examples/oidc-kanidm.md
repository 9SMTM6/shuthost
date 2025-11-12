# OIDC Authentication with Kanidm

OIDC authentication in ShutHost has only been tested with Kanidm. The following steps describe how to set up OIDC login using Kanidm:

## Steps

1. **Create the OAuth2 application in Kanidm:**
   ```sh
   kanidm system oauth2 create shuthost "Shuthost" https://shuthost.example.com
   kanidm system oauth2 add-redirect-url shuthost https://shuthost.example.com/oidc/callback
   kanidm group create shuthost_users
   kanidm group add-members shuthost_users <groups or users allowed to see UI>
   kanidm system oauth2 update-scope-map shuthost shuthost_users profile openid
   kanidm system oauth2 show-basic-secret shuthost
   ```
   - Replace `https://shuthost.example.com` with your actual ShutHost URL.
   - Note the client secret output by the last command.

2. **Configure ShutHost to use OIDC:**
   In your `coordinator_config.toml`:
   ```toml
   [server.auth.oidc]
   issuer = "https://kanidm.example.com/oauth2/openid/shuthost"
   client_id = "shuthost"
   client_secret = "<the secret from above>"
   ```
   - Adjust the `issuer` URL to match your Kanidm instance.
   - Use the client secret from the previous step.

3. **Restart ShutHost** to apply the changes.

> With this setup, users will be able to log in to the WebUI using their Kanidm credentials. Only members of the `shuthost_users` group will have access.