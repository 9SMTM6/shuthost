# OIDC Authentication (with Kanidm)

**Requirements:** The OIDC provider must support both PKCE (Proof Key for Code Exchange) and OIDC discovery. ShutHost implements OIDC as a confidential client, requiring a client secret.

This guide provides a tested setup example using Kanidm, but the configuration should be applicable to other OIDC providers like Keycloak, Auth0, or Google.

## Steps

1. **Create the OAuth2 application in your OIDC provider:**
   ```sh
   # Example CLI commands for Kanidm (add shuthost to your OIDC provider with equivalent data)
   kanidm system oauth2 create shuthost "Shuthost" https://shuthost.example.com
   kanidm system oauth2 add-redirect-url shuthost https://shuthost.example.com/oidc/callback
   kanidm group create shuthost_users
   kanidm group add-members shuthost_users <groups or users allowed to see UI>
   kanidm system oauth2 update-scope-map shuthost shuthost_users profile openid
   kanidm system oauth2 show-basic-secret shuthost
   ```
   - Replace `https://shuthost.example.com` with your actual ShutHost URL.
   - Note the client secret output by the last command.
   - If your provider is different, consult your OIDC provider's documentation for how to setup an OIDC client.

2. **Configure ShutHost to use OIDC:**
   In your `coordinator_config.toml`:
   ```toml
   [server.auth.oidc]
   issuer = "https://kanidm.example.com/oauth2/openid/shuthost"
   client_id = "shuthost"
   client_secret = "<the secret from above>"
   ```
   - Adjust the `issuer` URL to match your OIDC provider.
   - Use the client secret from the previous step.

3. **Restart ShutHost** to apply the changes.

> With this setup, users will be able to log in to the WebUI using their OIDC credentials. Access can be restricted based on provider-specific group memberships.

## Scopes

ShutHost requests the following OIDC scopes by default:
- `openid`: Required for OIDC authentication, provides the ID token.
- `profile`: Grants access to basic user profile information (e.g., name, email). This is not currently required by ShutHost but is requested for potential future expansion.

To limit scopes to only what's necessary, explicitly set them in your config:
```toml
[server.auth.oidc]
scopes = ["openid"]
```

## Security Notes

- Store the client secret securely (e.g., in environment variables or a secrets manager) and never commit it to version control.
- Regularly rotate the client secret via your OIDC provider's management interface to minimize exposure risks.
- Ensure HTTPS is used for all OIDC endpoints to protect against man-in-the-middle attacks.

## Troubleshooting

- **Logout not working**: ShutHost relies on the OIDC provider for session management; users may need to log out directly from the provider.
- **General issues**: Check the logs of your OIDC provider for detailed error information.
- **Login fails with "invalid_client"**: Verify the client ID and secret match exactly what your OIDC provider provided.
- **Redirect URI mismatch**: Ensure the redirect URL in your OIDC provider matches `https://shuthost.example.com/oidc/callback`.
- **Discovery issues**: Confirm your OIDC provider supports OIDC discovery at the issuer URL.
- **Access denied**: Check that the user has appropriate permissions in your OIDC provider (e.g., group membership).

For provider-specific details, refer to your OIDC provider's documentation.
