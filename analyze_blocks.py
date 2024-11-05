import json

def analyze_transaction_data(file_path):
    """
    Analyzes the transactions in a blockchain JSON file.

    Args:
        file_path (str): Path to the JSON file containing block data.

    Returns:
        dict: Analysis results including total transactions, unique transactions, and average transactions per block.
    """
    with open(file_path, 'r') as file:
        data = json.load(file)
    
    # Calculate transaction counts and unique transaction counts excluding genesis block
    transaction_counts = [len(block) for block in data[1:]]
    total_transactions = sum(transaction_counts)
    unique_transactions = len(set(tx for block in data[1:] for tx in block))
    
    # Average transactions per block, excluding genesis
    avg_transactions_per_block = total_transactions / len(transaction_counts) if transaction_counts else 0
    
    return {
        "total_transactions": total_transactions,
        "unique_transactions": unique_transactions,
        "avg_transactions_per_block": avg_transactions_per_block
    }

def analyze_files(file_paths):
    """
    Analyzes multiple blockchain JSON files.

    Args:
        file_paths (list): List of file paths to JSON files.

    Returns:
        dict: Analysis results for each file.
    """
    return {f'Node {i+1}': analyze_transaction_data(path) for i, path in enumerate(file_paths)}

# Replace with the paths to your JSON files
file_paths = [
    '7000.json',
    '7001.json',
    '7002.json'
]

# Run analysis and print results
results = analyze_files(file_paths)
for node, result in results.items():
    print(f"{node}:")
    print(f"  Total Transactions: {result['total_transactions']}")
    print(f"  Unique Transactions: {result['unique_transactions']}")
    print(f"  Avg Transactions per Block: {result['avg_transactions_per_block']:.2f}")
    print()
