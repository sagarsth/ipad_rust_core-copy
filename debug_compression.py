#!/usr/bin/env python3
"""
Manual Debug Session for Compression System
Checks which document and photo files were compressed in the iPad simulator.
"""

import sqlite3
import os
import json
from datetime import datetime
from pathlib import Path
import hashlib
import subprocess

class CompressionDebugger:
    def __init__(self, db_path=None, storage_path=None):
        # Try to find the database automatically
        if db_path is None:
            db_path = self.find_database()
        
        if storage_path is None:
            storage_path = self.find_storage_path()
            
        self.db_path = db_path
        self.storage_path = storage_path
        
        print(f"🔍 Database: {self.db_path}")
        print(f"📁 Storage: {self.storage_path}")
        
    def find_database(self):
        """Find the SQLite database file"""
        possible_paths = [
            "./storage/actionaid.db",
            "../storage/actionaid.db", 
            "./actionaid.db",
            os.path.expanduser("~/Library/Developer/CoreSimulator/Devices/*/data/Containers/Data/Application/*/Documents/actionaid.db"),
        ]
        
        for pattern in possible_paths:
            if '*' in pattern:
                # Use glob for wildcard patterns
                import glob
                matches = glob.glob(pattern)
                if matches:
                    return matches[0]
            elif os.path.exists(pattern):
                return pattern
                
        # Try finding with environment variable
        ios_docs = os.environ.get('IOS_DOCUMENTS_DIR')
        if ios_docs:
            db_path = os.path.join(ios_docs, 'actionaid.db')
            if os.path.exists(db_path):
                return db_path
        
        raise FileNotFoundError("Could not find actionaid.db database file")
    
    def find_storage_path(self):
        """Find the storage directory"""
        possible_paths = [
            "./storage",
            "../storage",
            os.path.expanduser("~/Library/Developer/CoreSimulator/Devices/*/data/Containers/Data/Application/*/Documents"),
        ]
        
        for pattern in possible_paths:
            if '*' in pattern:
                import glob
                matches = glob.glob(pattern)
                if matches:
                    return matches[0]
            elif os.path.exists(pattern):
                return pattern
                
        ios_docs = os.environ.get('IOS_DOCUMENTS_DIR')
        if ios_docs:
            return ios_docs
            
        return "./storage"  # fallback
    
    def get_compression_overview(self):
        """Get overall compression statistics"""
        print("\n" + "="*80)
        print("🔍 COMPRESSION OVERVIEW")
        print("="*80)
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        # Get compression status breakdown
        cursor.execute("""
            SELECT 
                compression_status,
                COUNT(*) as count,
                SUM(size_bytes) as total_original_size,
                SUM(CASE WHEN compressed_size_bytes IS NOT NULL THEN compressed_size_bytes ELSE 0 END) as total_compressed_size
            FROM media_documents 
            WHERE file_path != 'ERROR'
            GROUP BY compression_status
            ORDER BY count DESC
        """)
        
        results = cursor.fetchall()
        
        print("\n📊 Status Breakdown:")
        total_docs = 0
        total_original = 0
        total_compressed = 0
        
        for status, count, orig_size, comp_size in results:
            total_docs += count
            total_original += orig_size or 0
            total_compressed += comp_size or 0
            
            print(f"   {status:<12}: {count:>3} documents | {self.format_bytes(orig_size or 0):>10} original | {self.format_bytes(comp_size or 0):>10} compressed")
        
        print(f"\n📈 Totals: {total_docs} documents | {self.format_bytes(total_original)} original | {self.format_bytes(total_compressed)} compressed")
        
        if total_compressed > 0 and total_original > 0:
            savings = total_original - total_compressed
            percentage = (savings / total_original) * 100
            print(f"💾 Space Saved: {self.format_bytes(savings)} ({percentage:.1f}%)")
        
        conn.close()
    
    def get_compressed_documents(self):
        """Get all documents that have been compressed"""
        print("\n" + "="*80)
        print("✅ SUCCESSFULLY COMPRESSED DOCUMENTS")
        print("="*80)
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        cursor.execute("""
            SELECT 
                id,
                original_filename,
                mime_type,
                size_bytes,
                compressed_size_bytes,
                file_path,
                compressed_file_path,
                created_at,
                related_table
            FROM media_documents 
            WHERE compression_status = 'completed' 
                AND compressed_file_path IS NOT NULL
                AND file_path != 'ERROR'
            ORDER BY created_at DESC
        """)
        
        results = cursor.fetchall()
        
        if not results:
            print("❌ No compressed documents found!")
            conn.close()
            return
        
        print(f"\n🎯 Found {len(results)} compressed documents:")
        
        for doc in results:
            doc_id, filename, mime, orig_size, comp_size, orig_path, comp_path, created, table = doc
            
            savings = orig_size - (comp_size or orig_size)
            percentage = (savings / orig_size) * 100 if orig_size > 0 else 0
            
            print(f"\n📄 {filename}")
            print(f"   🆔 ID: {doc_id[:8]}...")
            print(f"   🗂️ Type: {mime} ({table})")
            print(f"   📏 Size: {self.format_bytes(orig_size)} → {self.format_bytes(comp_size or 0)}")
            print(f"   💾 Saved: {self.format_bytes(savings)} ({percentage:.1f}%)")
            print(f"   📁 Original: {orig_path}")
            print(f"   🗜️ Compressed: {comp_path}")
            print(f"   📅 Created: {created}")
            
            # Check if files actually exist
            self.check_file_existence(orig_path, comp_path)
        
        conn.close()
    
    def get_failed_compressions(self):
        """Get documents that failed compression"""
        print("\n" + "="*80)
        print("❌ FAILED COMPRESSIONS")
        print("="*80)
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        cursor.execute("""
            SELECT 
                id,
                original_filename,
                mime_type,
                size_bytes,
                file_path,
                error_message,
                has_error
            FROM media_documents 
            WHERE compression_status = 'failed' 
                OR has_error = 1
                AND file_path != 'ERROR'
            ORDER BY created_at DESC
        """)
        
        results = cursor.fetchall()
        
        if not results:
            print("✅ No failed compressions found!")
        else:
            print(f"\n🚨 Found {len(results)} failed documents:")
            for doc in results:
                doc_id, filename, mime, size, path, error, has_error = doc
                print(f"\n📄 {filename}")
                print(f"   🆔 ID: {doc_id[:8]}...")
                print(f"   🗂️ Type: {mime}")
                print(f"   📏 Size: {self.format_bytes(size)}")
                print(f"   ❌ Error: {error or 'Unknown error'}")
                print(f"   📁 Path: {path}")
        
        conn.close()
    
    def get_compression_queue_status(self):
        """Check the compression queue"""
        print("\n" + "="*80)
        print("🔄 COMPRESSION QUEUE STATUS")
        print("="*80)
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        # Check if compression_queue table exists
        cursor.execute("""
            SELECT name FROM sqlite_master WHERE type='table' AND name='compression_queue'
        """)
        
        if not cursor.fetchone():
            print("ℹ️ No compression_queue table found")
            conn.close()
            return
        
        cursor.execute("""
            SELECT 
                document_id,
                priority,
                status,
                queued_at,
                started_at,
                completed_at,
                error_message,
                attempts
            FROM compression_queue 
            ORDER BY queued_at DESC
            LIMIT 20
        """)
        
        results = cursor.fetchall()
        
        if not results:
            print("📭 Queue is empty")
        else:
            print(f"\n📋 Found {len(results)} queue entries (showing latest 20):")
            for entry in results:
                doc_id, priority, status, queued, started, completed, error, attempts = entry
                print(f"\n🔄 Document: {doc_id[:8]}...")
                print(f"   🚦 Status: {status}")
                print(f"   ⚡ Priority: {priority}")
                print(f"   📅 Queued: {queued}")
                print(f"   🏃 Started: {started or 'Not started'}")
                print(f"   ✅ Completed: {completed or 'Not completed'}")
                print(f"   🔄 Attempts: {attempts}")
                if error:
                    print(f"   ❌ Error: {error}")
        
        conn.close()
    
    def check_file_existence(self, orig_path, comp_path):
        """Check if files exist on disk"""
        storage_base = Path(self.storage_path)
        
        # Check original file
        orig_full_path = storage_base / orig_path
        orig_exists = orig_full_path.exists()
        orig_size = orig_full_path.stat().st_size if orig_exists else 0
        
        print(f"   📁 Original exists: {'✅' if orig_exists else '❌'} ({self.format_bytes(orig_size)})")
        
        # Check compressed file
        if comp_path:
            comp_full_path = storage_base / comp_path
            comp_exists = comp_full_path.exists()
            comp_size = comp_full_path.stat().st_size if comp_exists else 0
            
            print(f"   🗜️ Compressed exists: {'✅' if comp_exists else '❌'} ({self.format_bytes(comp_size)})")
            
            if comp_exists and orig_exists:
                savings = orig_size - comp_size
                percentage = (savings / orig_size) * 100 if orig_size > 0 else 0
                print(f"   💾 Actual savings: {self.format_bytes(savings)} ({percentage:.1f}%)")
    
    def scan_storage_directory(self):
        """Scan the storage directory to see what files exist"""
        print("\n" + "="*80)
        print("📁 STORAGE DIRECTORY SCAN")
        print("="*80)
        
        storage_path = Path(self.storage_path)
        
        if not storage_path.exists():
            print(f"❌ Storage directory does not exist: {storage_path}")
            return
        
        print(f"\n🔍 Scanning: {storage_path}")
        
        # Check for original and compressed subdirectories
        original_dir = storage_path / "original"
        compressed_dir = storage_path / "compressed"
        
        print(f"\n📂 Original directory: {'✅' if original_dir.exists() else '❌'}")
        if original_dir.exists():
            self.scan_directory(original_dir, "📄")
        
        print(f"\n📂 Compressed directory: {'✅' if compressed_dir.exists() else '❌'}")
        if compressed_dir.exists():
            self.scan_directory(compressed_dir, "🗜️")
    
    def scan_directory(self, directory, icon):
        """Recursively scan a directory"""
        file_count = 0
        total_size = 0
        
        for root, dirs, files in os.walk(directory):
            for file in files:
                if file.startswith('.'):  # Skip hidden files
                    continue
                    
                file_path = Path(root) / file
                file_size = file_path.stat().st_size
                file_count += 1
                total_size += file_size
                
                # Show first few files as examples
                if file_count <= 5:
                    rel_path = file_path.relative_to(directory)
                    print(f"   {icon} {rel_path} ({self.format_bytes(file_size)})")
        
        if file_count > 5:
            print(f"   ... and {file_count - 5} more files")
        
        print(f"   📊 Total: {file_count} files, {self.format_bytes(total_size)}")
    
    def get_document_types_analysis(self):
        """Analyze compression by document types"""
        print("\n" + "="*80)
        print("📊 DOCUMENT TYPES COMPRESSION ANALYSIS")
        print("="*80)
        
        conn = sqlite3.connect(self.db_path)
        cursor = conn.cursor()
        
        cursor.execute("""
            SELECT 
                dt.name as type_name,
                dt.compression_level,
                dt.compression_method,
                dt.min_size_for_compression,
                COUNT(md.id) as doc_count,
                SUM(CASE WHEN md.compression_status = 'completed' THEN 1 ELSE 0 END) as compressed_count,
                SUM(CASE WHEN md.compression_status = 'failed' THEN 1 ELSE 0 END) as failed_count,
                SUM(CASE WHEN md.compression_status = 'skipped' THEN 1 ELSE 0 END) as skipped_count,
                AVG(md.size_bytes) as avg_size,
                SUM(md.size_bytes) as total_original_size,
                SUM(CASE WHEN md.compressed_size_bytes IS NOT NULL THEN md.compressed_size_bytes ELSE 0 END) as total_compressed_size
            FROM document_types dt
            LEFT JOIN media_documents md ON dt.id = md.type_id AND md.file_path != 'ERROR'
            GROUP BY dt.id, dt.name
            ORDER BY doc_count DESC
        """)
        
        results = cursor.fetchall()
        
        print(f"\n📋 Document Type Analysis:")
        
        for row in results:
            type_name, comp_level, comp_method, min_size, doc_count, compressed, failed, skipped, avg_size, total_orig, total_comp = row
            
            if doc_count == 0:
                continue
                
            print(f"\n📂 {type_name}")
            print(f"   🗜️ Compression: Level {comp_level}, Method: {comp_method}")
            print(f"   📏 Min size for compression: {self.format_bytes(min_size or 0)}")
            print(f"   📊 Documents: {doc_count} total")
            print(f"   ✅ Compressed: {compressed}")
            print(f"   ❌ Failed: {failed}")
            print(f"   ⏭️ Skipped: {skipped}")
            print(f"   📐 Average size: {self.format_bytes(avg_size or 0)}")
            
            if total_comp > 0 and total_orig > 0:
                savings = total_orig - total_comp
                percentage = (savings / total_orig) * 100
                print(f"   💾 Total savings: {self.format_bytes(savings)} ({percentage:.1f}%)")
        
        conn.close()
    
    def format_bytes(self, bytes_val):
        """Format bytes into human readable format"""
        if bytes_val is None:
            return "0 B"
        
        for unit in ['B', 'KB', 'MB', 'GB']:
            if bytes_val < 1024.0:
                return f"{bytes_val:.1f} {unit}"
            bytes_val /= 1024.0
        return f"{bytes_val:.1f} TB"
    
    def run_full_debug(self):
        """Run complete debug session"""
        print("🔍 COMPRESSION DEBUG SESSION")
        print("="*80)
        print(f"📅 Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        
        try:
            self.get_compression_overview()
            self.get_compressed_documents()
            self.get_failed_compressions()
            self.get_compression_queue_status()
            self.get_document_types_analysis()
            self.scan_storage_directory()
            
            print("\n" + "="*80)
            print("✅ DEBUG SESSION COMPLETED")
            print("="*80)
            
        except Exception as e:
            print(f"\n❌ Debug session failed: {e}")
            import traceback
            traceback.print_exc()

if __name__ == "__main__":
    import sys
    
    db_path = sys.argv[1] if len(sys.argv) > 1 else None
    storage_path = sys.argv[2] if len(sys.argv) > 2 else None
    
    debugger = CompressionDebugger(db_path, storage_path)
    debugger.run_full_debug() 