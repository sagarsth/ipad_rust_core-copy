//
//  PreviewHelper.swift
//  SwiftUI_ActionAid
//
//  Created for legacy test view compatibility
//

import Foundation

#if DEBUG
struct PreviewHelper {
    static func setupPreviewEnvironmentAsync() async -> AuthenticationState {
        // Return a dummy auth state for preview purposes
        let authState = AuthenticationState()
        return authState
    }
}
#endif 