// Code generated - DO NOT EDIT.
// This file is a generated binding and any manual changes will be lost.

package bindings

import (
	"errors"
	"math/big"
	"strings"

	ethereum "github.com/ethereum/go-ethereum"
	"github.com/ethereum/go-ethereum/accounts/abi"
	"github.com/ethereum/go-ethereum/accounts/abi/bind"
	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core/types"
	"github.com/ethereum/go-ethereum/event"
)

// Reference imports to suppress errors if they are not otherwise used.
var (
	_ = errors.New
	_ = big.NewInt
	_ = strings.NewReader
	_ = ethereum.NotFound
	_ = bind.Bind
	_ = common.Big1
	_ = types.BloomLookup
	_ = event.NewSubscription
	_ = abi.ConvertType
)

// OPSuccinctDisputeGameFactoryMetaData contains all meta data concerning the OPSuccinctDisputeGameFactory contract.
var OPSuccinctDisputeGameFactoryMetaData = &bind.MetaData{
	ABI: "[{\"type\":\"constructor\",\"inputs\":[{\"name\":\"_owner\",\"type\":\"address\",\"internalType\":\"address\"},{\"name\":\"_gameImpl\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"create\",\"inputs\":[{\"name\":\"_rootClaim\",\"type\":\"bytes32\",\"internalType\":\"bytes32\"},{\"name\":\"_l2BlockNumber\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"_l1BlockNumber\",\"type\":\"uint256\",\"internalType\":\"uint256\"},{\"name\":\"_proof\",\"type\":\"bytes\",\"internalType\":\"bytes\"}],\"outputs\":[],\"stateMutability\":\"payable\"},{\"type\":\"function\",\"name\":\"gameImpl\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"owner\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"address\",\"internalType\":\"address\"}],\"stateMutability\":\"view\"},{\"type\":\"function\",\"name\":\"setImplementation\",\"inputs\":[{\"name\":\"_implementation\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"transferOwnership\",\"inputs\":[{\"name\":\"_owner\",\"type\":\"address\",\"internalType\":\"address\"}],\"outputs\":[],\"stateMutability\":\"nonpayable\"},{\"type\":\"function\",\"name\":\"version\",\"inputs\":[],\"outputs\":[{\"name\":\"\",\"type\":\"string\",\"internalType\":\"string\"}],\"stateMutability\":\"view\"}]",
	Bin: "0x6080604052348015600f57600080fd5b506040516106d23803806106d2833981016040819052602c916077565b600080546001600160a01b039384166001600160a01b0319918216179091556001805492909316911617905560a5565b80516001600160a01b0381168114607257600080fd5b919050565b60008060408385031215608957600080fd5b609083605c565b9150609c60208401605c565b90509250929050565b61061e806100b46000396000f3fe6080604052600436106100555760003560e01c806354fd4d501461005a5780636dd7af7f146100a75780638da5cb5b146100bc578063ce9c4172146100f4578063d784d42614610114578063f2fde38b14610134575b600080fd5b34801561006657600080fd5b506100916040518060400160405280600b81526020016a76312e302e302d6265746160a81b81525081565b60405161009e9190610405565b60405180910390f35b6100ba6100b536600461042e565b610154565b005b3480156100c857600080fd5b506000546100dc906001600160a01b031681565b6040516001600160a01b03909116815260200161009e565b34801561010057600080fd5b506001546100dc906001600160a01b031681565b34801561012057600080fd5b506100ba61012f3660046104fc565b610213565b34801561014057600080fd5b506100ba61014f3660046104fc565b610268565b60006101b633866000801b87878760405160200161017493929190610525565b60408051601f19818403018152908290526101949493929160200161054d565b60408051601f198184030181529190526001546001600160a01b0316906102b4565b9050806001600160a01b0316638129fc1c346040518263ffffffff1660e01b81526004016000604051808303818588803b1580156101f357600080fd5b505af1158015610207573d6000803e3d6000fd5b50505050505050505050565b6000546001600160a01b031633146102465760405162461bcd60e51b815260040161023d90610593565b60405180910390fd5b600180546001600160a01b0319166001600160a01b0392909216919091179055565b6000546001600160a01b031633146102925760405162461bcd60e51b815260040161023d90610593565b600080546001600160a01b0319166001600160a01b0392909216919091179055565b60006102c2600084846102c9565b9392505050565b600060608203516040830351602084035184518060208701018051600283016c5af43d3d93803e606057fd5bf3895289600d8a035278593da1005b363d3d373d3d3d3d610000806062363936013d738160481b1760218a03527f9e4ac34f21c619cefc926c8bd93b54bf5a39c7ab2127a895af1cc0691d7e3dff603a8a035272fd6100003d81600a3d39f336602c57343d527f6062820160781b1761ff9e82106059018a03528060f01b8352606c8101604c8a038cf0975050866103955763301164256000526004601cfd5b90528552601f19850152603f19840152605f199092019190915292915050565b60005b838110156103d05781810151838201526020016103b8565b50506000910152565b600081518084526103f18160208601602086016103b5565b601f01601f19169290920160200192915050565b6020815260006102c260208301846103d9565b634e487b7160e01b600052604160045260246000fd5b6000806000806080858703121561044457600080fd5b843593506020850135925060408501359150606085013567ffffffffffffffff8082111561047157600080fd5b818701915087601f83011261048557600080fd5b81358181111561049757610497610418565b604051601f8201601f19908116603f011681019083821181831017156104bf576104bf610418565b816040528281528a60208487010111156104d857600080fd5b82602086016020830137600060208483010152809550505050505092959194509250565b60006020828403121561050e57600080fd5b81356001600160a01b03811681146102c257600080fd5b83815282602082015260606040820152600061054460608301846103d9565b95945050505050565b6bffffffffffffffffffffffff198560601b168152836014820152826034820152600082516105838160548501602087016103b5565b9190910160540195945050505050565b60208082526035908201527f4f5053756363696e63744469737075746547616d65466163746f72793a206361604082015274363632b91034b9903737ba103a34329037bbb732b960591b60608201526080019056fea264697066735822122001974e8e2d11769ae5d93def519b4fc28e55cf559f862ba61a53d7cb6abc511c64736f6c63430008190033",
}

// OPSuccinctDisputeGameFactoryABI is the input ABI used to generate the binding from.
// Deprecated: Use OPSuccinctDisputeGameFactoryMetaData.ABI instead.
var OPSuccinctDisputeGameFactoryABI = OPSuccinctDisputeGameFactoryMetaData.ABI

// OPSuccinctDisputeGameFactoryBin is the compiled bytecode used for deploying new contracts.
// Deprecated: Use OPSuccinctDisputeGameFactoryMetaData.Bin instead.
var OPSuccinctDisputeGameFactoryBin = OPSuccinctDisputeGameFactoryMetaData.Bin

// DeployOPSuccinctDisputeGameFactory deploys a new Ethereum contract, binding an instance of OPSuccinctDisputeGameFactory to it.
func DeployOPSuccinctDisputeGameFactory(auth *bind.TransactOpts, backend bind.ContractBackend, _owner common.Address, _gameImpl common.Address) (common.Address, *types.Transaction, *OPSuccinctDisputeGameFactory, error) {
	parsed, err := OPSuccinctDisputeGameFactoryMetaData.GetAbi()
	if err != nil {
		return common.Address{}, nil, nil, err
	}
	if parsed == nil {
		return common.Address{}, nil, nil, errors.New("GetABI returned nil")
	}

	address, tx, contract, err := bind.DeployContract(auth, *parsed, common.FromHex(OPSuccinctDisputeGameFactoryBin), backend, _owner, _gameImpl)
	if err != nil {
		return common.Address{}, nil, nil, err
	}
	return address, tx, &OPSuccinctDisputeGameFactory{OPSuccinctDisputeGameFactoryCaller: OPSuccinctDisputeGameFactoryCaller{contract: contract}, OPSuccinctDisputeGameFactoryTransactor: OPSuccinctDisputeGameFactoryTransactor{contract: contract}, OPSuccinctDisputeGameFactoryFilterer: OPSuccinctDisputeGameFactoryFilterer{contract: contract}}, nil
}

// OPSuccinctDisputeGameFactory is an auto generated Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactory struct {
	OPSuccinctDisputeGameFactoryCaller     // Read-only binding to the contract
	OPSuccinctDisputeGameFactoryTransactor // Write-only binding to the contract
	OPSuccinctDisputeGameFactoryFilterer   // Log filterer for contract events
}

// OPSuccinctDisputeGameFactoryCaller is an auto generated read-only Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactoryCaller struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// OPSuccinctDisputeGameFactoryTransactor is an auto generated write-only Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactoryTransactor struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// OPSuccinctDisputeGameFactoryFilterer is an auto generated log filtering Go binding around an Ethereum contract events.
type OPSuccinctDisputeGameFactoryFilterer struct {
	contract *bind.BoundContract // Generic contract wrapper for the low level calls
}

// OPSuccinctDisputeGameFactorySession is an auto generated Go binding around an Ethereum contract,
// with pre-set call and transact options.
type OPSuccinctDisputeGameFactorySession struct {
	Contract     *OPSuccinctDisputeGameFactory // Generic contract binding to set the session for
	CallOpts     bind.CallOpts                 // Call options to use throughout this session
	TransactOpts bind.TransactOpts             // Transaction auth options to use throughout this session
}

// OPSuccinctDisputeGameFactoryCallerSession is an auto generated read-only Go binding around an Ethereum contract,
// with pre-set call options.
type OPSuccinctDisputeGameFactoryCallerSession struct {
	Contract *OPSuccinctDisputeGameFactoryCaller // Generic contract caller binding to set the session for
	CallOpts bind.CallOpts                       // Call options to use throughout this session
}

// OPSuccinctDisputeGameFactoryTransactorSession is an auto generated write-only Go binding around an Ethereum contract,
// with pre-set transact options.
type OPSuccinctDisputeGameFactoryTransactorSession struct {
	Contract     *OPSuccinctDisputeGameFactoryTransactor // Generic contract transactor binding to set the session for
	TransactOpts bind.TransactOpts                       // Transaction auth options to use throughout this session
}

// OPSuccinctDisputeGameFactoryRaw is an auto generated low-level Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactoryRaw struct {
	Contract *OPSuccinctDisputeGameFactory // Generic contract binding to access the raw methods on
}

// OPSuccinctDisputeGameFactoryCallerRaw is an auto generated low-level read-only Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactoryCallerRaw struct {
	Contract *OPSuccinctDisputeGameFactoryCaller // Generic read-only contract binding to access the raw methods on
}

// OPSuccinctDisputeGameFactoryTransactorRaw is an auto generated low-level write-only Go binding around an Ethereum contract.
type OPSuccinctDisputeGameFactoryTransactorRaw struct {
	Contract *OPSuccinctDisputeGameFactoryTransactor // Generic write-only contract binding to access the raw methods on
}

// NewOPSuccinctDisputeGameFactory creates a new instance of OPSuccinctDisputeGameFactory, bound to a specific deployed contract.
func NewOPSuccinctDisputeGameFactory(address common.Address, backend bind.ContractBackend) (*OPSuccinctDisputeGameFactory, error) {
	contract, err := bindOPSuccinctDisputeGameFactory(address, backend, backend, backend)
	if err != nil {
		return nil, err
	}
	return &OPSuccinctDisputeGameFactory{OPSuccinctDisputeGameFactoryCaller: OPSuccinctDisputeGameFactoryCaller{contract: contract}, OPSuccinctDisputeGameFactoryTransactor: OPSuccinctDisputeGameFactoryTransactor{contract: contract}, OPSuccinctDisputeGameFactoryFilterer: OPSuccinctDisputeGameFactoryFilterer{contract: contract}}, nil
}

// NewOPSuccinctDisputeGameFactoryCaller creates a new read-only instance of OPSuccinctDisputeGameFactory, bound to a specific deployed contract.
func NewOPSuccinctDisputeGameFactoryCaller(address common.Address, caller bind.ContractCaller) (*OPSuccinctDisputeGameFactoryCaller, error) {
	contract, err := bindOPSuccinctDisputeGameFactory(address, caller, nil, nil)
	if err != nil {
		return nil, err
	}
	return &OPSuccinctDisputeGameFactoryCaller{contract: contract}, nil
}

// NewOPSuccinctDisputeGameFactoryTransactor creates a new write-only instance of OPSuccinctDisputeGameFactory, bound to a specific deployed contract.
func NewOPSuccinctDisputeGameFactoryTransactor(address common.Address, transactor bind.ContractTransactor) (*OPSuccinctDisputeGameFactoryTransactor, error) {
	contract, err := bindOPSuccinctDisputeGameFactory(address, nil, transactor, nil)
	if err != nil {
		return nil, err
	}
	return &OPSuccinctDisputeGameFactoryTransactor{contract: contract}, nil
}

// NewOPSuccinctDisputeGameFactoryFilterer creates a new log filterer instance of OPSuccinctDisputeGameFactory, bound to a specific deployed contract.
func NewOPSuccinctDisputeGameFactoryFilterer(address common.Address, filterer bind.ContractFilterer) (*OPSuccinctDisputeGameFactoryFilterer, error) {
	contract, err := bindOPSuccinctDisputeGameFactory(address, nil, nil, filterer)
	if err != nil {
		return nil, err
	}
	return &OPSuccinctDisputeGameFactoryFilterer{contract: contract}, nil
}

// bindOPSuccinctDisputeGameFactory binds a generic wrapper to an already deployed contract.
func bindOPSuccinctDisputeGameFactory(address common.Address, caller bind.ContractCaller, transactor bind.ContractTransactor, filterer bind.ContractFilterer) (*bind.BoundContract, error) {
	parsed, err := OPSuccinctDisputeGameFactoryMetaData.GetAbi()
	if err != nil {
		return nil, err
	}
	return bind.NewBoundContract(address, *parsed, caller, transactor, filterer), nil
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _OPSuccinctDisputeGameFactory.Contract.OPSuccinctDisputeGameFactoryCaller.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.OPSuccinctDisputeGameFactoryTransactor.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.OPSuccinctDisputeGameFactoryTransactor.contract.Transact(opts, method, params...)
}

// Call invokes the (constant) contract method with params as input values and
// sets the output to result. The result type might be a single field for simple
// returns, a slice of interfaces for anonymous returns and a struct for named
// returns.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCallerRaw) Call(opts *bind.CallOpts, result *[]interface{}, method string, params ...interface{}) error {
	return _OPSuccinctDisputeGameFactory.Contract.contract.Call(opts, result, method, params...)
}

// Transfer initiates a plain transaction to move funds to the contract, calling
// its default method if one is available.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactorRaw) Transfer(opts *bind.TransactOpts) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.contract.Transfer(opts)
}

// Transact invokes the (paid) contract method with params as input values.
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactorRaw) Transact(opts *bind.TransactOpts, method string, params ...interface{}) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.contract.Transact(opts, method, params...)
}

// GameImpl is a free data retrieval call binding the contract method 0xce9c4172.
//
// Solidity: function gameImpl() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCaller) GameImpl(opts *bind.CallOpts) (common.Address, error) {
	var out []interface{}
	err := _OPSuccinctDisputeGameFactory.contract.Call(opts, &out, "gameImpl")

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// GameImpl is a free data retrieval call binding the contract method 0xce9c4172.
//
// Solidity: function gameImpl() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) GameImpl() (common.Address, error) {
	return _OPSuccinctDisputeGameFactory.Contract.GameImpl(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// GameImpl is a free data retrieval call binding the contract method 0xce9c4172.
//
// Solidity: function gameImpl() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCallerSession) GameImpl() (common.Address, error) {
	return _OPSuccinctDisputeGameFactory.Contract.GameImpl(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// Owner is a free data retrieval call binding the contract method 0x8da5cb5b.
//
// Solidity: function owner() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCaller) Owner(opts *bind.CallOpts) (common.Address, error) {
	var out []interface{}
	err := _OPSuccinctDisputeGameFactory.contract.Call(opts, &out, "owner")

	if err != nil {
		return *new(common.Address), err
	}

	out0 := *abi.ConvertType(out[0], new(common.Address)).(*common.Address)

	return out0, err

}

// Owner is a free data retrieval call binding the contract method 0x8da5cb5b.
//
// Solidity: function owner() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) Owner() (common.Address, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Owner(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// Owner is a free data retrieval call binding the contract method 0x8da5cb5b.
//
// Solidity: function owner() view returns(address)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCallerSession) Owner() (common.Address, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Owner(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// Version is a free data retrieval call binding the contract method 0x54fd4d50.
//
// Solidity: function version() view returns(string)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCaller) Version(opts *bind.CallOpts) (string, error) {
	var out []interface{}
	err := _OPSuccinctDisputeGameFactory.contract.Call(opts, &out, "version")

	if err != nil {
		return *new(string), err
	}

	out0 := *abi.ConvertType(out[0], new(string)).(*string)

	return out0, err

}

// Version is a free data retrieval call binding the contract method 0x54fd4d50.
//
// Solidity: function version() view returns(string)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) Version() (string, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Version(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// Version is a free data retrieval call binding the contract method 0x54fd4d50.
//
// Solidity: function version() view returns(string)
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryCallerSession) Version() (string, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Version(&_OPSuccinctDisputeGameFactory.CallOpts)
}

// Create is a paid mutator transaction binding the contract method 0x6dd7af7f.
//
// Solidity: function create(bytes32 _rootClaim, uint256 _l2BlockNumber, uint256 _l1BlockNumber, bytes _proof) payable returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactor) Create(opts *bind.TransactOpts, _rootClaim [32]byte, _l2BlockNumber *big.Int, _l1BlockNumber *big.Int, _proof []byte) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.contract.Transact(opts, "create", _rootClaim, _l2BlockNumber, _l1BlockNumber, _proof)
}

// Create is a paid mutator transaction binding the contract method 0x6dd7af7f.
//
// Solidity: function create(bytes32 _rootClaim, uint256 _l2BlockNumber, uint256 _l1BlockNumber, bytes _proof) payable returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) Create(_rootClaim [32]byte, _l2BlockNumber *big.Int, _l1BlockNumber *big.Int, _proof []byte) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Create(&_OPSuccinctDisputeGameFactory.TransactOpts, _rootClaim, _l2BlockNumber, _l1BlockNumber, _proof)
}

// Create is a paid mutator transaction binding the contract method 0x6dd7af7f.
//
// Solidity: function create(bytes32 _rootClaim, uint256 _l2BlockNumber, uint256 _l1BlockNumber, bytes _proof) payable returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactorSession) Create(_rootClaim [32]byte, _l2BlockNumber *big.Int, _l1BlockNumber *big.Int, _proof []byte) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.Create(&_OPSuccinctDisputeGameFactory.TransactOpts, _rootClaim, _l2BlockNumber, _l1BlockNumber, _proof)
}

// SetImplementation is a paid mutator transaction binding the contract method 0xd784d426.
//
// Solidity: function setImplementation(address _implementation) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactor) SetImplementation(opts *bind.TransactOpts, _implementation common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.contract.Transact(opts, "setImplementation", _implementation)
}

// SetImplementation is a paid mutator transaction binding the contract method 0xd784d426.
//
// Solidity: function setImplementation(address _implementation) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) SetImplementation(_implementation common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.SetImplementation(&_OPSuccinctDisputeGameFactory.TransactOpts, _implementation)
}

// SetImplementation is a paid mutator transaction binding the contract method 0xd784d426.
//
// Solidity: function setImplementation(address _implementation) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactorSession) SetImplementation(_implementation common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.SetImplementation(&_OPSuccinctDisputeGameFactory.TransactOpts, _implementation)
}

// TransferOwnership is a paid mutator transaction binding the contract method 0xf2fde38b.
//
// Solidity: function transferOwnership(address _owner) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactor) TransferOwnership(opts *bind.TransactOpts, _owner common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.contract.Transact(opts, "transferOwnership", _owner)
}

// TransferOwnership is a paid mutator transaction binding the contract method 0xf2fde38b.
//
// Solidity: function transferOwnership(address _owner) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactorySession) TransferOwnership(_owner common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.TransferOwnership(&_OPSuccinctDisputeGameFactory.TransactOpts, _owner)
}

// TransferOwnership is a paid mutator transaction binding the contract method 0xf2fde38b.
//
// Solidity: function transferOwnership(address _owner) returns()
func (_OPSuccinctDisputeGameFactory *OPSuccinctDisputeGameFactoryTransactorSession) TransferOwnership(_owner common.Address) (*types.Transaction, error) {
	return _OPSuccinctDisputeGameFactory.Contract.TransferOwnership(&_OPSuccinctDisputeGameFactory.TransactOpts, _owner)
}
