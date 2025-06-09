import SwiftUI

struct CoreInitializationView: View {
    @EnvironmentObject var authState: AuthenticationState
    @AppStorage("hasCompletedInitialSetup") private var hasCompletedInitialSetup = false
    
    @State private var statusMessage = "Initializing..."
    @State private var showError = false
    @State private var errorMessage = ""

    // FFI Handlers
    private let coreHandler = CoreFFIHandler()
    private let authHandler = AuthFFIHandler()

    var body: some View {
        VStack(spacing: 20) {
            Spacer()
            Text("ActionAid")
                .font(.largeTitle.bold())
                .foregroundColor(.accentColor)
            
            if showError {
                Image(systemName: "xmark.octagon.fill")
                    .font(.system(size: 50))
                    .foregroundColor(.red)
                Text("Initialization Failed")
                    .font(.headline)
                Text(errorMessage)
                    .font(.caption)
                    .multilineTextAlignment(.center)
                    .padding()
            } else {
                ProgressView()
                    .scaleEffect(1.5)
                Text(statusMessage)
                    .font(.headline)
                    .foregroundColor(.secondary)
            }
            
            Spacer()
        }
        .padding()
        .task {
            await performInitialSetup()
        }
    }
    
    private func performInitialSetup() async {
        do {
            statusMessage = "Preparing storage..."
            let storagePath = try coreHandler.prepareStorage()
            
            statusMessage = "Initializing core library..."
            try await coreHandler.initializeLibrary(storagePath: storagePath)
            
            statusMessage = "Creating default accounts..."
            _ = try await authHandler.initializeDefaultAccounts(token: "init_setup")

            statusMessage = "Loading test data..."
            _ = try await authHandler.initializeTestData(token: "init_setup")
            
            statusMessage = "Logging in as administrator..."
            let credentials = Credentials(email: "admin@example.com", password: "Admin123!")
            let loginResponse = try await authHandler.login(credentials: credentials).get()
            
            // Update the global authentication state
            authState.updateLastLoggedInUser(
                userId: loginResponse.user_id,
                role: loginResponse.role,
                email: credentials.email,
                token: loginResponse.access_token
            )
            
            statusMessage = "Setup complete!"
            // Mark setup as complete to show the main app view
            hasCompletedInitialSetup = true
            
        } catch {
            errorMessage = error.localizedDescription
            showError = true
            statusMessage = "Error"
        }
    }
}

#Preview {
    CoreInitializationView()
        .environmentObject(AuthenticationState())
} 