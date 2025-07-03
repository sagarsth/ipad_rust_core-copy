//
//  QuickLookView.swift
//  ActionAid SwiftUI
//
//  SwiftUI wrapper for QuickLook document preview
//

import SwiftUI
import QuickLook

// MARK: - Identifiable URL wrapper for QuickLook

struct IdentifiableURL: Identifiable {
    let id = UUID()
    let url: URL
}

// MARK: - QuickLook Support

/// SwiftUI wrapper for QLPreviewController to display documents
struct QuickLookView: UIViewControllerRepresentable {
    let url: URL
    let onDismiss: (() -> Void)?
    
    init(url: URL, onDismiss: (() -> Void)? = nil) {
        self.url = url
        self.onDismiss = onDismiss
    }
    
    func makeUIViewController(context: Context) -> QLPreviewController {
        let controller = QLPreviewController()
        controller.dataSource = context.coordinator
        controller.delegate = context.coordinator
        return controller
    }
    
    func updateUIViewController(_ uiViewController: QLPreviewController, context: Context) {
        // Check if the URL has changed and needs reload
        if context.coordinator.url != url {
            context.coordinator.url = url
            uiViewController.reloadData()
        }
    }
    
    func makeCoordinator() -> Coordinator {
        Coordinator(url: url, onDismiss: onDismiss)
    }
    
    class Coordinator: NSObject, QLPreviewControllerDataSource, QLPreviewControllerDelegate {
        var url: URL
        let onDismiss: (() -> Void)?
        
        init(url: URL, onDismiss: (() -> Void)? = nil) {
            self.url = url
            self.onDismiss = onDismiss
        }
        
        func numberOfPreviewItems(in controller: QLPreviewController) -> Int { 1 }
        
        func previewController(_ controller: QLPreviewController, previewItemAt index: Int) -> QLPreviewItem {
            // Ensure the file exists before attempting to preview
            guard FileManager.default.fileExists(atPath: url.path) else {
                print("📖 [QUICKLOOK] File does not exist at path: \(url.path)")
                // Return the URL anyway - QuickLook will show an appropriate error
                return url as QLPreviewItem
            }
            
            print("📖 [QUICKLOOK] Previewing file: \(url.lastPathComponent)")
            return url as QLPreviewItem
        }
        
        func previewControllerWillDismiss(_ controller: QLPreviewController) {
            print("📖 [QUICKLOOK] Document viewer will dismiss")
            onDismiss?()
        }
    }
} 