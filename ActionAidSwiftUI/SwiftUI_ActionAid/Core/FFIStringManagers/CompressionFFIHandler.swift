//
//  CompressionFFIHandler.swift
//  SwiftUI_ActionAid
//
//  Created by Wentai Li on 11/25/23.
//

import Foundation

/// A handler that provides a Swift-friendly interface to the Rust `compression` FFI functions.
class CompressionFFIHandler {
    private let queue = DispatchQueue(label: "com.actionaid.compression.ffi", qos: .userInitiated)
    private let jsonEncoder = JSONEncoder()
    private let jsonDecoder = JSONDecoder()

    // MARK: - Document Compression
    
    /// Compresses a document based on the request configuration
    func compressDocument(request: CompressDocumentRequest) async -> Result<CompressionResultResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_compress_document(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Gets the current status of the compression queue
    func getQueueStatus() async -> Result<CompressionQueueStatusResponse, Error> {
        await executeOperation { resultPtr in
            compression_get_queue_status(resultPtr)
        }
    }
    
    /// Queues a document for compression without performing the operation immediately
    func queueDocumentForCompression(request: QueueDocumentRequest) async -> Result<Void, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeVoidOperation {
                jsonPayload.withCString { cJson in
                    compression_queue_document(cJson)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
         /// Cancels compression for a specific document
     func cancelCompression(request: DocumentIdRequest) async -> Result<CancelResponse, Error> {
         do {
             let jsonPayload = try encode(request)
             return await executeOperation { resultPtr in
                 jsonPayload.withCString { cJson in
                     compression_cancel(cJson, resultPtr)
                 }
             }
         } catch {
             return .failure(error)
         }
     }
    
    /// Gets compression statistics and metrics
    func getCompressionStats() async -> Result<CompressionStatsResponse, Error> {
        await executeOperation { resultPtr in
            compression_get_stats(resultPtr)
        }
    }
    
    /// Gets the compression status for a specific document
    func getDocumentCompressionStatus(request: DocumentIdRequest) async -> Result<CompressionStatus, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_get_document_status(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Updates the priority of a document in the compression queue
    func updateCompressionPriority(request: UpdatePriorityRequest) async -> Result<UpdatePriorityResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_update_priority(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Updates the priority of multiple documents in the compression queue
    func bulkUpdateCompressionPriority(request: BulkUpdatePriorityRequest) async -> Result<BulkUpdatePriorityResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_bulk_update_priority(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Checks if a specific document is currently being used in compression
    func isDocumentInUse(request: DocumentIdRequest) async -> Result<IsDocumentInUseResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_is_document_in_use(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Retrieves queue entries with optional filtering
    func getQueueEntries(request: GetQueueEntriesRequest) async -> Result<[String], Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_get_queue_entries(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
         /// Gets the current compression configuration
     func getCompressionConfig() async -> Result<CompressionConfig, Error> {
         await executeOperation { resultPtr in
             compression_get_default_config(resultPtr)
         }
     }
    
    /// Validates a compression configuration
    func validateCompressionConfig(request: ValidateConfigRequest) async -> Result<ValidateConfigResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_validate_config(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Retries compression for a failed document
    func retryFailedCompression(request: DocumentIdRequest) async -> Result<RetryFailedResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_retry_failed(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Retries compression for all failed documents
    func retryAllFailedCompressions() async -> Result<RetryAllFailedResponse, Error> {
        await executeOperation { resultPtr in
            compression_retry_all_failed(resultPtr)
        }
    }
    
    /// Processes the compression queue immediately
    func processQueueNow() async -> Result<Void, Error> {
        await executeVoidOperation {
            compression_process_queue_now()
        }
    }
    
    /// Gets supported compression methods
    func getSupportedMethods(request: GetSupportedMethodsRequest) async -> Result<GetSupportedMethodsResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_get_supported_methods(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    /// Gets the compression history for a document
    func getDocumentHistory(request: DocumentIdRequest) async -> Result<DocumentHistoryResponse, Error> {
        do {
            let jsonPayload = try encode(request)
            return await executeOperation { resultPtr in
                jsonPayload.withCString { cJson in
                    compression_get_document_history(cJson, resultPtr)
                }
            }
        } catch {
            return .failure(error)
        }
    }
    
    // MARK: - Private Helpers
    
    private func encode<T: Encodable>(_ value: T) throws -> String {
        let data = try jsonEncoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw FFIError.stringConversionFailed
        }
        return string
    }

    /// Executes an FFI operation that is expected to return a JSON string, which is then decoded into a `Decodable` type.
    private func executeOperation<T: Decodable>(
        _ operation: @escaping (UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>) -> CInt
    ) async -> Result<T, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                let ffiResult = FFIHelper.execute(
                    call: operation,
                    parse: { responseString in
                        guard let data = responseString.data(using: .utf8) else {
                            throw FFIError.stringConversionFailed
                        }
                        return try self.jsonDecoder.decode(T.self, from: data)
                    },
                    free: compression_free
                )
                
                if let value = ffiResult.value {
                    continuation.resume(returning: .success(value))
                } else if let error = ffiResult.error {
                    continuation.resume(returning: .failure(FFIError.rustError(error)))
                } else {
                    continuation.resume(returning: .failure(FFIError.unknown))
                }
            }
        }
    }

    /// Executes an FFI operation that does not return any data.
    private func executeVoidOperation(
        _ operation: @escaping () -> CInt
    ) async -> Result<Void, Error> {
        await withCheckedContinuation { continuation in
            queue.async {
                let status = operation()
                if status == 0 {
                    continuation.resume(returning: .success(()))
                } else {
                    let error = FFIHelper.getLastError()
                    continuation.resume(returning: .failure(FFIError.rustError(error)))
                }
            }
        }
    }
} 